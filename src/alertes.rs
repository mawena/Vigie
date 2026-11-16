//! Décide quand alerter (durée minimale, rappels, aggravation, retour à la normale)
//! et envoie les messages sur les canaux configurés (ntfy, Telegram, e-mail).

use crate::config::{Alertes, Config};
use crate::modele::{Mesure, Niveau, duree};
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Un événement à annoncer.
#[derive(Debug, Clone, PartialEq)]
pub struct Evenement {
    pub cle: String,
    pub niveau: Niveau,
    /// Vrai pour un retour à la normale.
    pub retour: bool,
    pub rappel: bool,
    pub texte: String,
}

#[derive(Debug)]
struct Suivi {
    /// Dernier niveau observé et nombre de tournées consécutives à ce niveau.
    observe: Niveau,
    tours: u32,
    /// Niveau annoncé (Ok : rien d'annoncé en cours).
    annonce: Niveau,
    /// Début du problème et date du dernier message (secondes Unix).
    depuis: i64,
    dernier_envoi: i64,
}

/// Mémoire des problèmes en cours.
#[derive(Debug, Default)]
pub struct Veilleur {
    suivis: HashMap<String, Suivi>,
}

impl Veilleur {
    /// Compare une tournée à la précédente ; renvoie ce qu'il faut annoncer.
    pub fn evaluer(&mut self, c: &Alertes, mesures: &[Mesure], maintenant: i64) -> Vec<Evenement> {
        let seuil = if c.attention { Niveau::Attention } else { Niveau::Critique };
        let mut evenements = Vec::new();
        // Les objets qui ne sont plus mesurés (conteneur supprimé, sonde retirée) sont oubliés.
        self.suivis.retain(|cle, _| mesures.iter().any(|m| &m.cle == cle));
        for m in mesures {
            // En dessous du seuil d'alerte, le niveau compte comme « Ok ».
            let niveau = if m.niveau >= seuil { m.niveau } else { Niveau::Ok };
            let s = self.suivis.entry(m.cle.clone()).or_insert(Suivi {
                observe: niveau,
                tours: 0,
                annonce: Niveau::Ok,
                depuis: maintenant,
                dernier_envoi: 0,
            });
            if niveau == s.observe {
                s.tours = s.tours.saturating_add(1);
            } else {
                if s.observe == Niveau::Ok {
                    s.depuis = maintenant;
                }
                s.observe = niveau;
                s.tours = 1;
            }
            if s.tours < c.duree_min {
                continue;
            }
            let objet = format!("{} {}", m.categorie, m.nom);
            if niveau == Niveau::Ok {
                if s.annonce != Niveau::Ok {
                    s.annonce = Niveau::Ok;
                    if c.retour {
                        evenements.push(Evenement {
                            cle: m.cle.clone(),
                            niveau: Niveau::Ok,
                            retour: true,
                            rappel: false,
                            texte: format!("{objet} : retour à la normale ({})", m.valeur),
                        });
                    }
                }
                continue;
            }
            let texte = |prefixe: &str| {
                let mut t = format!("{prefixe}{objet} : {}", m.valeur);
                if !m.detail.is_empty() {
                    t.push_str(&format!(" — {}", m.detail));
                }
                t
            };
            if niveau > s.annonce {
                s.annonce = niveau;
                s.dernier_envoi = maintenant;
                evenements.push(Evenement { cle: m.cle.clone(), niveau, retour: false, rappel: false, texte: texte("") });
            } else if niveau < s.annonce {
                // Amélioration partielle (critique → attention) : pas de message, rappel au niveau courant.
                s.annonce = niveau;
            } else if c.rappel > 0 && maintenant - s.dernier_envoi >= c.rappel as i64 * 60 {
                s.dernier_envoi = maintenant;
                let prefixe = format!("toujours, depuis {} : ", duree(maintenant - s.depuis));
                evenements.push(Evenement { cle: m.cle.clone(), niveau, retour: false, rappel: true, texte: texte(&prefixe) });
            }
        }
        evenements
    }

    /// Problèmes annoncés et toujours en cours.
    pub fn en_cours(&self) -> usize {
        self.suivis.values().filter(|s| s.annonce != Niveau::Ok).count()
    }
}

/// Message prêt à partir : un seul par tournée, qui regroupe les événements.
#[derive(Debug, PartialEq)]
pub struct Message {
    pub titre: String,
    pub corps: String,
    /// Le plus grave des événements (Ok si ce ne sont que des retours à la normale).
    pub niveau: Niveau,
}

pub fn composer(machine: &str, evenements: &[Evenement]) -> Option<Message> {
    if evenements.is_empty() {
        return None;
    }
    let niveau = evenements.iter().filter(|e| !e.retour).map(|e| e.niveau).max().unwrap_or(Niveau::Ok);
    let problemes = evenements.iter().filter(|e| !e.retour).count();
    let titre = match (problemes, evenements.len()) {
        (0, 1) => format!("{machine} : retour à la normale"),
        (0, n) => format!("{machine} : {n} retours à la normale"),
        (1, 1) => format!("{machine} : {}", niveau.libelle().to_lowercase()),
        (p, _) => format!("{machine} : {p} problème{}", if p > 1 { "s" } else { "" }),
    };
    let mut lignes: Vec<&Evenement> = evenements.iter().collect();
    lignes.sort_by(|a, b| b.niveau.cmp(&a.niveau).then(a.retour.cmp(&b.retour)));
    let corps = lignes
        .iter()
        .map(|e| {
            let marque = match (e.retour, e.niveau) {
                (true, _) | (_, Niveau::Ok) => "✅",
                (_, Niveau::Attention) => "⚠️",
                (_, Niveau::Critique) => "🔴",
            };
            format!("{marque} {}", e.texte)
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(Message { titre, corps, niveau })
}

/// Envoie sur tous les canaux configurés ; renvoie les erreurs (un canal en panne n'empêche pas les autres).
pub fn envoyer(c: &Config, m: &Message) -> Result<usize, Vec<String>> {
    let mut erreurs = Vec::new();
    let mut envoyes = 0;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!("vigie/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    if let Some(n) = &c.ntfy {
        let (priorite, etiquette) = match m.niveau {
            Niveau::Critique => (5, "rotating_light"),
            Niveau::Attention => (4, "warning"),
            Niveau::Ok => (3, "white_check_mark"),
        };
        // Publication JSON à la racine du serveur : accents sans souci d'en-têtes HTTP.
        let corps = serde_json::json!({
            "topic": n.sujet,
            "title": m.titre,
            "message": m.corps,
            "priority": priorite,
            "tags": [etiquette],
        });
        let mut req = agent.post(n.serveur.trim_end_matches('/')).header("Content-Type", "application/json");
        if let Some(j) = n.jeton.as_ref().filter(|j| !j.is_empty()) {
            req = req.header("Authorization", &format!("Bearer {j}"));
        }
        match req.send(corps.to_string()) {
            Ok(_) => envoyes += 1,
            Err(e) => erreurs.push(format!("ntfy : {e}")),
        }
    }
    if let Some(t) = &c.telegram {
        let corps = serde_json::json!({ "chat_id": t.chat, "text": format!("{}\n{}", m.titre, m.corps) });
        let url = format!("https://api.telegram.org/bot{}/sendMessage", t.jeton);
        match agent.post(&url).header("Content-Type", "application/json").send(corps.to_string()) {
            Ok(_) => envoyes += 1,
            // L'erreur de ureq contient l'URL, donc le jeton : on ne la recopie pas.
            Err(e) => erreurs.push(format!("telegram : envoi refusé ({})", nature(&e))),
        }
    }
    if let Some(co) = &c.courriel {
        match courriel(co, m) {
            Ok(()) => envoyes += 1,
            Err(e) => erreurs.push(format!("courriel : {e}")),
        }
    }
    if erreurs.is_empty() { Ok(envoyes) } else { Err(erreurs) }
}

fn nature(e: &ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(c) => format!("code {c}"),
        ureq::Error::Timeout(_) => "délai dépassé".into(),
        _ => "réseau".into(),
    }
}

fn courriel(c: &crate::config::Courriel, m: &Message) -> Result<(), String> {
    let sujet = encoder_entete(&m.titre);
    let texte = format!(
        "From: {}\nTo: {}\nSubject: {sujet}\nMIME-Version: 1.0\nContent-Type: text/plain; charset=utf-8\nContent-Transfer-Encoding: 8bit\n\n{}\n",
        c.de, c.a, m.corps
    );
    let mut enfant = Command::new(&c.commande)
        .args(["-t", "-i"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{} : {e}", c.commande))?;
    enfant.stdin.take().ok_or("stdin")?.write_all(texte.as_bytes()).map_err(|e| e.to_string())?;
    let sortie = enfant.wait_with_output().map_err(|e| e.to_string())?;
    if sortie.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&sortie.stderr).trim().to_string())
    }
}

/// En-tête de courriel en UTF-8 (RFC 2047).
fn encoder_entete(t: &str) -> String {
    use base64::Engine;
    if t.is_ascii() {
        t.to_string()
    } else {
        format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(cle: &str, niveau: Niveau) -> Mesure {
        Mesure::new(cle, "Disque", "/").niveau(niveau).valeur("x")
    }

    fn reglage() -> Alertes {
        Alertes { duree_min: 2, rappel: 60, retour: true, attention: true }
    }

    #[test]
    fn duree_min_rappel_retour() {
        let c = reglage();
        let mut v = Veilleur::default();
        assert!(v.evaluer(&c, &[m("d", Niveau::Ok)], 0).is_empty());
        // Un raté isolé : rien.
        assert!(v.evaluer(&c, &[m("d", Niveau::Critique)], 60).is_empty());
        assert!(v.evaluer(&c, &[m("d", Niveau::Ok)], 120).is_empty());
        // Deux tournées : alerte.
        assert!(v.evaluer(&c, &[m("d", Niveau::Critique)], 180).is_empty());
        let e = v.evaluer(&c, &[m("d", Niveau::Critique)], 240);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].niveau, Niveau::Critique);
        assert_eq!(v.en_cours(), 1);
        // Pas de répétition avant le rappel.
        assert!(v.evaluer(&c, &[m("d", Niveau::Critique)], 300).is_empty());
        let e = v.evaluer(&c, &[m("d", Niveau::Critique)], 240 + 3600);
        assert!(e[0].rappel && e[0].texte.starts_with("toujours, depuis 1 h"));
        // Retour à la normale après deux tournées.
        assert!(v.evaluer(&c, &[m("d", Niveau::Ok)], 4000).is_empty());
        let e = v.evaluer(&c, &[m("d", Niveau::Ok)], 4060);
        assert!(e[0].retour);
        assert_eq!(v.en_cours(), 0);
    }

    #[test]
    fn aggravation_et_seuil() {
        let mut c = reglage();
        c.duree_min = 1;
        let mut v = Veilleur::default();
        assert_eq!(v.evaluer(&c, &[m("d", Niveau::Attention)], 0).len(), 1);
        assert_eq!(v.evaluer(&c, &[m("d", Niveau::Critique)], 60).len(), 1);
        assert!(v.evaluer(&c, &[m("d", Niveau::Attention)], 120).is_empty());
        // Seulement le critique.
        c.attention = false;
        let mut v = Veilleur::default();
        assert!(v.evaluer(&c, &[m("d", Niveau::Attention)], 0).is_empty());
    }

    #[test]
    fn message_groupe() {
        let e = vec![
            Evenement { cle: "a".into(), niveau: Niveau::Ok, retour: true, rappel: false, texte: "A revenu".into() },
            Evenement { cle: "b".into(), niveau: Niveau::Critique, retour: false, rappel: false, texte: "B tombé".into() },
        ];
        let msg = composer("vps", &e).unwrap();
        assert_eq!(msg.titre, "vps : 1 problème");
        assert_eq!(msg.niveau, Niveau::Critique);
        assert!(msg.corps.starts_with("🔴 B tombé\n✅ A revenu"));
        assert!(composer("vps", &[]).is_none());
        assert_eq!(composer("vps", &e[..1]).unwrap().titre, "vps : retour à la normale");
    }

    #[test]
    fn entete() {
        assert_eq!(encoder_entete("ok"), "ok");
        assert_eq!(encoder_entete("é"), "=?UTF-8?B?w6k=?=");
    }
}
