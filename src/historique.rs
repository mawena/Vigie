//! Historique court dans SQLite : valeurs numériques des mesures (un point par pas) et événements
//! d'alerte. Purge automatique au-delà de la durée de conservation.

use crate::alertes::Evenement;
use crate::modele::Mesure;
use rusqlite::{Connection, params};
use std::path::Path;

pub struct Historique {
    base: Connection,
    dernier_point: i64,
    derniere_purge: i64,
}

impl Historique {
    pub fn ouvrir(chemin: &Path) -> Result<Historique, String> {
        if let Some(d) = chemin.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
        }
        let base = Connection::open(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        base.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 2000;
             CREATE TABLE IF NOT EXISTS points (t INTEGER NOT NULL, cle TEXT NOT NULL, valeur REAL NOT NULL);
             CREATE INDEX IF NOT EXISTS points_cle_t ON points (cle, t);
             CREATE TABLE IF NOT EXISTS evenements (t INTEGER NOT NULL, cle TEXT NOT NULL, niveau TEXT NOT NULL, texte TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS evenements_t ON evenements (t);",
        )
        .map_err(|e| e.to_string())?;
        let dernier_point = base.query_row("SELECT COALESCE(MAX(t), 0) FROM points", [], |r| r.get(0)).unwrap_or(0);
        Ok(Historique { base, dernier_point, derniere_purge: 0 })
    }

    /// Enregistre les valeurs numériques si le pas est écoulé ; purge une fois par heure.
    pub fn enregistrer(&mut self, mesures: &[Mesure], t: i64, pas: u64, garder_jours: u64) -> Result<(), String> {
        if t - self.dernier_point >= pas as i64 {
            let tx = self.base.transaction().map_err(|e| e.to_string())?;
            {
                let mut req = tx.prepare_cached("INSERT INTO points (t, cle, valeur) VALUES (?1, ?2, ?3)").map_err(|e| e.to_string())?;
                for m in mesures {
                    if let Some(v) = m.metrique.filter(|v| v.is_finite()) {
                        req.execute(params![t, m.cle, v]).map_err(|e| e.to_string())?;
                    }
                }
            }
            tx.commit().map_err(|e| e.to_string())?;
            self.dernier_point = t;
        }
        if t - self.derniere_purge >= 3600 {
            let limite = t - garder_jours as i64 * 86400;
            self.base.execute("DELETE FROM points WHERE t < ?1", [limite]).map_err(|e| e.to_string())?;
            self.base.execute("DELETE FROM evenements WHERE t < ?1", [limite]).map_err(|e| e.to_string())?;
            self.derniere_purge = t;
        }
        Ok(())
    }

    pub fn noter(&self, evenements: &[Evenement], t: i64) -> Result<(), String> {
        for e in evenements {
            let niveau = if e.retour { "retour" } else { e.niveau.libelle() };
            self.base
                .execute("INSERT INTO evenements (t, cle, niveau, texte) VALUES (?1, ?2, ?3, ?4)", params![t, e.cle, niveau, e.texte])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Points d'une clé depuis `depuis`, dans l'ordre du temps.
    pub fn serie(&self, cle: &str, depuis: i64) -> Vec<(i64, f64)> {
        let Ok(mut req) = self.base.prepare_cached("SELECT t, valeur FROM points WHERE cle = ?1 AND t >= ?2 ORDER BY t") else {
            return Vec::new();
        };
        req.query_map(params![cle, depuis], |r| Ok((r.get(0)?, r.get(1)?)))
            .map(|l| l.flatten().collect())
            .unwrap_or_default()
    }

    /// Derniers événements, du plus récent au plus ancien : (t, niveau, texte).
    pub fn evenements(&self, n: usize) -> Vec<(i64, String, String)> {
        let Ok(mut req) = self.base.prepare_cached("SELECT t, niveau, texte FROM evenements ORDER BY t DESC, rowid DESC LIMIT ?1") else {
            return Vec::new();
        };
        req.query_map([n as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|l| l.flatten().collect())
            .unwrap_or_default()
    }
}

/// Jours avant d'atteindre 100 % d'après la pente (moindres carrés) des points donnés.
/// `None` si les points couvrent moins d'un jour ou si la courbe ne monte pas.
pub fn jours_avant_plein(points: &[(i64, f64)]) -> Option<f64> {
    if points.len() < 12 {
        return None;
    }
    let (t0, t1) = (points.first()?.0, points.last()?.0);
    if t1 - t0 < 86400 {
        return None;
    }
    let n = points.len() as f64;
    let xs: Vec<f64> = points.iter().map(|(t, _)| (*t - t0) as f64 / 86400.0).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = points.iter().map(|(_, v)| v).sum::<f64>() / n;
    let (mut num, mut den) = (0.0, 0.0);
    for (x, (_, y)) in xs.iter().zip(points) {
        num += (x - mx) * (y - my);
        den += (x - mx) * (x - mx);
    }
    if den == 0.0 {
        return None;
    }
    let pente = num / den; // points de % par jour
    if pente < 0.01 {
        return None;
    }
    let actuel = points.last()?.1;
    Some(((100.0 - actuel) / pente).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::Niveau;

    #[test]
    fn points_evenements_purge() {
        let f = std::env::temp_dir().join(format!("vigie-hist-{}.db", std::process::id()));
        let mut h = Historique::ouvrir(&f).unwrap();
        let m = [Mesure::new("disque:/", "Disque", "/").metrique(50.0), Mesure::new("x", "X", "x")];
        h.enregistrer(&m, 1000, 300, 30).unwrap();
        h.enregistrer(&m, 1100, 300, 30).unwrap(); // pas pas encore écoulé
        h.enregistrer(&m, 1300, 300, 30).unwrap();
        assert_eq!(h.serie("disque:/", 0), vec![(1000, 50.0), (1300, 50.0)]);
        assert!(h.serie("x", 0).is_empty());
        let e = Evenement { cle: "d".into(), niveau: Niveau::Critique, retour: false, rappel: false, texte: "panne".into() };
        h.noter(&[e], 1300).unwrap();
        assert_eq!(h.evenements(5), vec![(1300, "CRITIQUE".into(), "panne".into())]);
        // 31 jours plus tard, tout est purgé (sauf le nouveau point).
        h.enregistrer(&m, 1300 + 31 * 86400, 300, 30).unwrap();
        assert_eq!(h.serie("disque:/", 0).len(), 1);
        assert!(h.evenements(5).is_empty());
        drop(h);
        for s in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{s}", f.display()));
        }
    }

    #[test]
    fn tendance() {
        // +1 % par jour depuis 60 % sur 3 jours : plein dans ~37 jours.
        let p: Vec<(i64, f64)> = (0..=36).map(|i| (i * 7200, 60.0 + i as f64 * 2.0 / 24.0)).collect();
        let j = jours_avant_plein(&p).unwrap();
        assert!((j - 37.0).abs() < 0.1, "{j}");
        let plat: Vec<(i64, f64)> = (0..=36).map(|i| (i * 7200, 60.0)).collect();
        assert!(jours_avant_plein(&plat).is_none());
        assert!(jours_avant_plein(&p[..5]).is_none());
    }
}
