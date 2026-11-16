//! Sonde des sauvegardes : âge du fichier le plus récent d'un dossier.

use crate::config::Sauvegarde;
use crate::modele::{Mesure, Niveau, duree, maintenant};
use std::path::Path;
use std::time::UNIX_EPOCH;

pub fn sauvegardes(liste: &[Sauvegarde]) -> Vec<Mesure> {
    liste.iter().map(sauvegarde).collect()
}

fn sauvegarde(c: &Sauvegarde) -> Mesure {
    let cle = format!("sauvegarde:{}", c.nom);
    let mut visites = 0;
    match plus_recent(Path::new(&c.chemin), 0, &mut visites) {
        Ok(Some(t)) => {
            let age = (maintenant() - t).max(0);
            let heures = age as f64 / 3600.0;
            let niveau = if heures > 2.0 * c.max_heures {
                Niveau::Critique
            } else if heures > c.max_heures {
                Niveau::Attention
            } else {
                Niveau::Ok
            };
            let m = Mesure::new(cle, "Sauvegarde", c.nom.as_str())
                .niveau(niveau)
                .valeur(format!("il y a {}", duree(age)))
                .metrique(heures);
            if niveau == Niveau::Ok { m } else { m.detail(format!("aucune sauvegarde récente dans {}", c.chemin)) }
        }
        Ok(None) => Mesure::new(cle, "Sauvegarde", c.nom.as_str())
            .niveau(Niveau::Critique)
            .valeur("aucune")
            .detail(format!("{} est vide", c.chemin)),
        Err(e) => Mesure::erreur(cle, "Sauvegarde", c.nom.as_str(), format!("{} : {e}", c.chemin)),
    }
}

/// Date de modification la plus récente (secondes Unix) parmi les fichiers, sous-dossiers compris
/// (profondeur 6, 100 000 entrées au plus pour rester léger).
fn plus_recent(chemin: &Path, profondeur: u32, visites: &mut u32) -> std::io::Result<Option<i64>> {
    let meta = std::fs::symlink_metadata(chemin)?;
    if meta.is_file() {
        let t = meta.modified()?.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        return Ok(Some(t));
    }
    if !meta.is_dir() || profondeur > 6 {
        return Ok(None);
    }
    let mut max = None;
    for e in std::fs::read_dir(chemin)? {
        *visites += 1;
        if *visites > 100_000 {
            break;
        }
        let Ok(e) = e else { continue };
        if let Ok(Some(t)) = plus_recent(&e.path(), profondeur + 1, visites) {
            max = max.max(Some(t));
        }
    }
    Ok(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_dossier() {
        let d = std::env::temp_dir().join(format!("vigie-test-{}", std::process::id()));
        std::fs::create_dir_all(d.join("sous")).unwrap();
        let c = Sauvegarde { nom: "t".into(), chemin: d.display().to_string(), max_heures: 1.0 };
        assert_eq!(sauvegarde(&c).niveau, Niveau::Critique); // vide
        std::fs::write(d.join("sous/archive"), b"x").unwrap();
        let m = sauvegarde(&c);
        assert_eq!(m.niveau, Niveau::Ok);
        assert!(m.metrique.unwrap() < 0.1);
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!(sauvegarde(&c).niveau, Niveau::Attention); // chemin absent : sonde en erreur
    }
}
