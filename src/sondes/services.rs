//! Sondes services : systemd, Docker (socket local, lecture seule), libvirt (virsh).

use super::commande;
use crate::config::{Docker, Libvirt, Systemd};
use crate::modele::{Mesure, Niveau};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

const DELAI: Duration = Duration::from_secs(15);

pub fn systemd(c: &Systemd) -> Vec<Mesure> {
    let mut mesures = Vec::new();
    if c.echecs {
        mesures.push(
            match commande("systemctl", &["--system", "list-units", "--state=failed", "--no-legend", "--plain", "--no-pager"], DELAI) {
                Ok(sortie) => {
                    let echecs: Vec<String> = sortie
                        .lines()
                        .filter_map(|l| l.split_whitespace().next())
                        .filter(|u| !c.ignorer.iter().any(|i| i == u))
                        .map(String::from)
                        .collect();
                    let m = Mesure::new("systemd:echecs", "systemd", "unités en échec").metrique(echecs.len() as f64);
                    if echecs.is_empty() {
                        m.valeur("aucune")
                    } else {
                        m.niveau(Niveau::Critique).valeur(format!("{} en échec", echecs.len())).detail(echecs.join(", "))
                    }
                }
                Err(e) => Mesure::erreur("systemd:echecs", "systemd", "unités en échec", e),
            },
        );
    }
    mesures.extend(unites(&c.unites, false));
    mesures.extend(unites(&c.unites_utilisateur, true));
    mesures
}

/// État des unités voulues actives.
fn unites(noms: &[String], utilisateur: bool) -> Vec<Mesure> {
    if noms.is_empty() {
        return Vec::new();
    }
    let prefixe = if utilisateur { "systemd-user" } else { "systemd" };
    let mut args = vec![if utilisateur { "--user" } else { "--system" }, "show", "-p", "Id,LoadState,ActiveState,SubState", "--"];
    args.extend(noms.iter().map(String::as_str));
    let sortie = match commande("systemctl", &args, DELAI) {
        Ok(s) => s,
        Err(e) => {
            return noms.iter().map(|n| Mesure::erreur(format!("{prefixe}:{n}"), "systemd", n.as_str(), &e)).collect();
        }
    };
    // Un bloc « Clé=valeur » par unité, dans l'ordre demandé, séparés par une ligne vide.
    let blocs: Vec<&str> = sortie.split("\n\n").filter(|b| !b.trim().is_empty()).collect();
    noms.iter()
        .enumerate()
        .map(|(i, nom)| {
            let bloc = blocs.get(i).copied().unwrap_or("");
            let champ = |k: &str| bloc.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('='))).unwrap_or("");
            let (charge, actif, sous) = (champ("LoadState"), champ("ActiveState"), champ("SubState"));
            let m = Mesure::new(format!("{prefixe}:{nom}"), "systemd", nom.as_str()).valeur(format!("{actif} ({sous})"));
            if charge == "not-found" {
                m.niveau(Niveau::Critique).valeur("introuvable").detail("unité inconnue de systemd")
            } else {
                match actif {
                    "active" => m,
                    "activating" | "reloading" | "deactivating" => m.niveau(Niveau::Attention),
                    _ => m.niveau(Niveau::Critique).detail("l'unité devrait être active"),
                }
            }
        })
        .collect()
}

pub fn docker(c: &Docker) -> Vec<Mesure> {
    let actif = c.actif.unwrap_or_else(|| Path::new(&c.socket).exists() || !c.conteneurs.is_empty());
    if !actif {
        return Vec::new();
    }
    let conteneurs = match lister_conteneurs(&c.socket) {
        Ok(l) => l,
        Err(e) => return vec![Mesure::erreur("docker", "Docker", "démon", e)],
    };
    evaluer_conteneurs(c, &conteneurs)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Conteneur {
    pub names: Vec<String>,
    pub state: String,
    pub status: String,
}

impl Conteneur {
    fn nom(&self) -> &str {
        self.names.first().map(|n| n.trim_start_matches('/')).unwrap_or("?")
    }
}

fn evaluer_conteneurs(c: &Docker, conteneurs: &[Conteneur]) -> Vec<Mesure> {
    let mut mesures = Vec::new();
    let en_marche = conteneurs.iter().filter(|x| x.state == "running").count();
    let mut resume = Mesure::new("docker", "Docker", "conteneurs")
        .valeur(format!("{en_marche} en marche sur {}", conteneurs.len()))
        .metrique(en_marche as f64);
    if c.signaler_malades {
        let malades: Vec<String> = conteneurs
            .iter()
            .filter(|x| x.state == "restarting" || x.status.contains("(unhealthy)"))
            .map(|x| format!("{} ({})", x.nom(), x.status))
            .collect();
        if !malades.is_empty() {
            resume = resume.niveau(Niveau::Attention).detail(format!("malades : {}", malades.join(", ")));
        }
    }
    mesures.push(resume);
    for voulu in &c.conteneurs {
        let m = Mesure::new(format!("docker:{voulu}"), "Docker", voulu.as_str());
        mesures.push(match conteneurs.iter().find(|x| x.nom() == voulu) {
            None => m.niveau(Niveau::Critique).valeur("absent").detail("aucun conteneur de ce nom"),
            Some(x) if x.state != "running" => m.niveau(Niveau::Critique).valeur(x.state.clone()).detail(x.status.clone()),
            Some(x) if x.status.contains("(unhealthy)") => m.niveau(Niveau::Attention).valeur("malade").detail(x.status.clone()),
            Some(x) => m.valeur(x.status.clone()),
        });
    }
    mesures
}

/// GET /containers/json?all=1 sur le socket de Docker (HTTP/1.0 : réponse non découpée).
fn lister_conteneurs(socket: &str) -> Result<Vec<Conteneur>, String> {
    let mut s = UnixStream::connect(socket).map_err(|e| format!("{socket} : {e}"))?;
    s.set_read_timeout(Some(DELAI)).ok();
    s.set_write_timeout(Some(DELAI)).ok();
    s.write_all(b"GET /containers/json?all=1 HTTP/1.0\r\nHost: docker\r\n\r\n").map_err(|e| e.to_string())?;
    let mut reponse = Vec::new();
    s.read_to_end(&mut reponse).map_err(|e| e.to_string())?;
    let texte = String::from_utf8_lossy(&reponse);
    let (tete, corps) = texte.split_once("\r\n\r\n").ok_or("réponse de Docker illisible")?;
    let statut = tete.split_whitespace().nth(1).unwrap_or("");
    if statut != "200" {
        return Err(format!("Docker répond {statut}"));
    }
    serde_json::from_str(corps).map_err(|e| format!("réponse de Docker illisible : {e}"))
}

pub fn libvirt(c: &Libvirt) -> Vec<Mesure> {
    if c.domaines.is_empty() {
        return Vec::new();
    }
    let sortie = match commande("virsh", &["-c", &c.uri, "list", "--all"], DELAI) {
        Ok(s) => s,
        Err(e) => return c.domaines.iter().map(|d| Mesure::erreur(format!("libvirt:{d}"), "libvirt", d.as_str(), &e)).collect(),
    };
    let etats = analyser_virsh(&sortie);
    c.domaines
        .iter()
        .map(|d| {
            let m = Mesure::new(format!("libvirt:{d}"), "libvirt", d.as_str());
            match etats.iter().find(|(n, _)| n == d) {
                None => m.niveau(Niveau::Critique).valeur("absente").detail("machine inconnue de libvirt"),
                Some((_, etat)) if etat == "running" => m.valeur("en marche"),
                Some((_, etat)) => m.niveau(Niveau::Critique).valeur(etat.clone()).detail("la machine devrait tourner"),
            }
        })
        .collect()
}

/// Lignes « Id  Nom  État » de `virsh list --all` (l'état peut faire deux mots : « shut off »).
fn analyser_virsh(sortie: &str) -> Vec<(String, String)> {
    sortie
        .lines()
        .skip_while(|l| !l.trim_start().starts_with('-'))
        .skip(1)
        .filter_map(|l| {
            let mut mots = l.split_whitespace();
            let _id = mots.next()?;
            let nom = mots.next()?.to_string();
            let etat = mots.collect::<Vec<_>>().join(" ");
            Some((nom, etat))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virsh() {
        let s = " Id   Name     State\n------------------------\n 1    web      running\n -    sauve    shut off\n\n";
        assert_eq!(analyser_virsh(s), vec![("web".into(), "running".into()), ("sauve".into(), "shut off".into())]);
    }

    #[test]
    fn conteneurs() {
        let json = r#"[{"Names":["/a"],"State":"running","Status":"Up 2 hours (healthy)","Id":"x"},
                      {"Names":["/b"],"State":"exited","Status":"Exited (1) 3 minutes ago"},
                      {"Names":["/c"],"State":"running","Status":"Up 1 hour (unhealthy)"}]"#;
        let l: Vec<Conteneur> = serde_json::from_str(json).unwrap();
        let c = Docker { conteneurs: vec!["a".into(), "b".into(), "z".into()], ..Docker::default() };
        let m = evaluer_conteneurs(&c, &l);
        assert_eq!(m[0].niveau, Niveau::Attention); // c malade
        assert_eq!(m[0].valeur, "2 en marche sur 3");
        assert_eq!(m[1].niveau, Niveau::Ok);
        assert_eq!(m[2].niveau, Niveau::Critique);
        assert_eq!(m[3].valeur, "absent");
    }
}
