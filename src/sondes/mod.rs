//! Les sondes. Une tournée lance toutes les sondes configurées et renvoie leurs mesures.

pub mod fichiers;
pub mod reseau;
pub mod services;
pub mod systeme;

use crate::config::Config;
use crate::modele::Mesure;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Lance toutes les sondes. Les sondes réseau tournent en parallèle (un fil chacune).
pub fn tournee(c: &Config) -> Vec<Mesure> {
    let mut mesures = Vec::new();
    mesures.extend(systeme::disques(&c.disque));
    if c.memoire.actif {
        mesures.push(systeme::memoire(&c.memoire));
    }
    if c.charge.actif {
        mesures.push(systeme::charge(&c.charge));
    }
    if c.temperature.actif {
        mesures.extend(systeme::temperature(&c.temperature));
    }
    mesures.extend(services::systemd(&c.systemd));
    mesures.extend(services::docker(&c.docker));
    mesures.extend(services::libvirt(&c.libvirt));
    mesures.extend(fichiers::sauvegardes(&c.sauvegarde));

    std::thread::scope(|s| {
        let mut fils = Vec::new();
        for h in &c.http {
            fils.push(s.spawn(move || reseau::http(h)));
        }
        for t in &c.tls {
            fils.push(s.spawn(move || reseau::tls(t)));
        }
        for d in &c.dns {
            fils.push(s.spawn(move || reseau::dns(d)));
        }
        for f in fils {
            if let Ok(m) = f.join() {
                mesures.push(m);
            }
        }
    });
    mesures
}

/// Lance une commande avec un délai maximal ; renvoie sa sortie standard si elle réussit.
pub fn commande(programme: &str, args: &[&str], delai: Duration) -> Result<String, String> {
    let mut enfant = Command::new(programme)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{programme} : {e}"))?;
    // Lecture dans des fils pour ne pas bloquer si la sortie remplit le tube.
    let mut sortie = enfant.stdout.take().expect("stdout");
    let mut erreur = enfant.stderr.take().expect("stderr");
    let fil_sortie = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = sortie.read_to_string(&mut s);
        s
    });
    let fil_erreur = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = erreur.read_to_string(&mut s);
        s
    });
    let debut = Instant::now();
    let statut = loop {
        match enfant.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if debut.elapsed() > delai => {
                let _ = enfant.kill();
                let _ = enfant.wait();
                return Err(format!("{programme} : délai dépassé"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("{programme} : {e}")),
        }
    };
    let s = fil_sortie.join().unwrap_or_default();
    let e = fil_erreur.join().unwrap_or_default();
    if statut.success() {
        Ok(s)
    } else {
        let e = e.trim();
        Err(format!("{programme} : {}", if e.is_empty() { statut.to_string() } else { e.lines().next().unwrap_or(e).to_string() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commande_ok_et_delai() {
        assert_eq!(commande("echo", &["salut"], Duration::from_secs(5)).unwrap(), "salut\n");
        assert!(commande("false", &[], Duration::from_secs(5)).is_err());
        assert!(commande("sleep", &["5"], Duration::from_millis(100)).unwrap_err().contains("délai"));
        assert!(commande("programme-qui-n-existe-pas", &[], Duration::from_secs(1)).is_err());
    }
}
