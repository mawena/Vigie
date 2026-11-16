//! Sondes système : disque, mémoire, charge, température. Lecture de /proc et /sys seulement.

use crate::config::{Disque, Seuils};
use crate::modele::{Mesure, Niveau, nombre};
use std::ffi::CString;

/// Occupation de chaque système de fichiers configuré.
pub fn disques(c: &Disque) -> Vec<Mesure> {
    c.chemins
        .iter()
        .map(|chemin| {
            let cle = format!("disque:{chemin}");
            match occupation(chemin) {
                Ok((pourcent, libre)) => Mesure::new(cle, "Disque", chemin.as_str())
                    .niveau(Niveau::par_seuils(pourcent, c.attention, c.critique))
                    .valeur(format!("{} %", nombre(pourcent, 0)))
                    .detail(format!("{} libres", octets(libre)))
                    .metrique(pourcent),
                Err(e) => Mesure::erreur(cle, "Disque", chemin.as_str(), e),
            }
        })
        .collect()
}

/// (% utilisé, octets libres pour un utilisateur), comme `df`.
pub fn occupation(chemin: &str) -> Result<(f64, u64), String> {
    let c = CString::new(chemin).map_err(|e| e.to_string())?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY : chemin terminé par un zéro, structure de la bonne taille.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let bloc = s.f_frsize as u64;
    let total = s.f_blocks as u64;
    let libres = s.f_bfree as u64;
    let dispo = s.f_bavail as u64;
    let utilises = total.saturating_sub(libres);
    let base = utilises + dispo;
    if base == 0 {
        return Ok((0.0, 0));
    }
    Ok((utilises as f64 * 100.0 / base as f64, dispo * bloc))
}

pub fn memoire(c: &Seuils) -> Mesure {
    match std::fs::read_to_string("/proc/meminfo").map_err(|e| e.to_string()).and_then(|t| analyser_meminfo(&t)) {
        Ok((total, dispo)) => {
            let pourcent = (total - dispo) as f64 * 100.0 / total as f64;
            Mesure::new("memoire", "Mémoire", "RAM")
                .niveau(Niveau::par_seuils(pourcent, c.attention, c.critique))
                .valeur(format!("{} %", nombre(pourcent, 0)))
                .detail(format!("{} disponibles sur {}", octets(dispo), octets(total)))
                .metrique(pourcent)
        }
        Err(e) => Mesure::erreur("memoire", "Mémoire", "RAM", e),
    }
}

/// (total, disponible) en octets.
fn analyser_meminfo(texte: &str) -> Result<(u64, u64), String> {
    let champ = |nom: &str| {
        texte
            .lines()
            .find_map(|l| l.strip_prefix(nom))
            .and_then(|r| r.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
            .map(|k| k * 1024)
    };
    match (champ("MemTotal:"), champ("MemAvailable:")) {
        (Some(t), Some(d)) if t > 0 => Ok((t, d)),
        _ => Err("/proc/meminfo illisible".into()),
    }
}

pub fn charge(c: &Seuils) -> Mesure {
    let coeurs = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    match std::fs::read_to_string("/proc/loadavg") {
        Ok(t) => {
            let v: Vec<f64> = t.split_whitespace().take(3).filter_map(|x| x.parse().ok()).collect();
            if v.len() < 3 {
                return Mesure::erreur("charge", "Charge", "processeur", "/proc/loadavg illisible");
            }
            let par_coeur = v[1] / coeurs;
            Mesure::new("charge", "Charge", "processeur")
                .niveau(Niveau::par_seuils(par_coeur, c.attention, c.critique))
                .valeur(format!("{} / cœur", nombre(par_coeur, 2)))
                .detail(format!("{} {} {} sur {} cœurs", nombre(v[0], 2), nombre(v[1], 2), nombre(v[2], 2), coeurs))
                .metrique(par_coeur)
        }
        Err(e) => Mesure::erreur("charge", "Charge", "processeur", e),
    }
}

/// Zone thermique la plus chaude ; rien s'il n'y a pas de capteur.
pub fn temperature(c: &Seuils) -> Option<Mesure> {
    let mut max: Option<(f64, String)> = None;
    for e in std::fs::read_dir("/sys/class/thermal").ok()?.flatten() {
        let nom = e.file_name().to_string_lossy().to_string();
        if !nom.starts_with("thermal_zone") {
            continue;
        }
        let Some(t) = std::fs::read_to_string(e.path().join("temp")).ok().and_then(|s| s.trim().parse::<f64>().ok()) else {
            continue;
        };
        let t = t / 1000.0;
        let type_ = std::fs::read_to_string(e.path().join("type")).map(|s| s.trim().to_string()).unwrap_or(nom);
        if max.as_ref().is_none_or(|(m, _)| t > *m) {
            max = Some((t, type_));
        }
    }
    let (t, zone) = max?;
    Some(
        Mesure::new("temperature", "Température", "processeur")
            .niveau(Niveau::par_seuils(t, c.attention, c.critique))
            .valeur(format!("{} °C", nombre(t, 0)))
            .detail(format!("zone {zone}"))
            .metrique(t),
    )
}

/// Taille lisible : « 12,3 Go ».
pub fn octets(n: u64) -> String {
    let unites = ["o", "Ko", "Mo", "Go", "To"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < unites.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{n} o") } else { format!("{} {}", nombre(v, 1), unites[i]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo() {
        let t = "MemTotal:       1000 kB\nMemFree:  10 kB\nMemAvailable:    250 kB\n";
        assert_eq!(analyser_meminfo(t).unwrap(), (1024000, 256000));
        assert!(analyser_meminfo("rien").is_err());
    }

    #[test]
    fn tailles() {
        assert_eq!(octets(512), "512 o");
        assert_eq!(octets(1536), "1,5 Ko");
        assert_eq!(octets(5 * 1024 * 1024 * 1024), "5,0 Go");
    }

    #[test]
    fn disque_racine() {
        let (p, _) = occupation("/").unwrap();
        assert!((0.0..=100.0).contains(&p));
        assert!(occupation("/chemin/absent").is_err());
    }
}
