//! Machines distantes, vues par le relais de Ricochet (API opérateur, jeton en lecture) :
//! connexion de l'agent, disques et mémoire envoyés par l'agent. Marche pour un PC sans IP publique.

use crate::config::{Config, Ricochet};
use crate::modele::{Mesure, Niveau, duree, maintenant, nombre};
use crate::sondes::systeme::octets;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct VueMachine {
    pub id: String,
    pub nom: Option<String>,
    pub connectee: bool,
    pub vue_le: Option<i64>,
    pub etat: Option<EtatMachine>,
    pub etat_recu_le: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct EtatMachine {
    pub memoire_utilisee: u64,
    pub memoire_totale: u64,
    #[serde(default)]
    pub disques: Vec<EtatDisque>,
}

#[derive(Debug, Deserialize)]
pub struct EtatDisque {
    pub point_montage: String,
    pub utilise: u64,
    pub total: u64,
}

pub fn ricochet(c: &Config) -> Vec<Mesure> {
    let Some(r) = &c.ricochet else { return Vec::new() };
    match lister(r) {
        Ok(machines) => evaluer(c, r, &machines, maintenant()),
        Err(e) => vec![Mesure::erreur("ricochet", "Ricochet", "relais", e)],
    }
}

fn lister(r: &Ricochet) -> Result<Vec<VueMachine>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(concat!("vigie/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let url = format!("{}/api/machines", r.relais.trim_end_matches('/'));
    let mut reponse = agent
        .get(&url)
        .header("Authorization", &format!("Bearer {}", r.jeton))
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(401) => "jeton refusé par le relais".to_string(),
            e => format!("{url} : {e}"),
        })?;
    let texte = reponse.body_mut().with_config().limit(5_000_000).read_to_string().map_err(|e| e.to_string())?;
    serde_json::from_str(&texte).map_err(|e| format!("réponse du relais illisible : {e}"))
}

fn evaluer(c: &Config, r: &Ricochet, machines: &[VueMachine], t: i64) -> Vec<Mesure> {
    let mut mesures = Vec::new();
    for voulue in r.machines.iter().chain(r.toujours_en_ligne.iter().filter(|m| !r.machines.contains(m))) {
        if !machines.iter().any(|m| &m.id == voulue) {
            mesures.push(
                Mesure::new(format!("ricochet:{voulue}"), "Machine", voulue.as_str())
                    .niveau(Niveau::Critique)
                    .valeur("inconnue")
                    .detail("machine absente du relais (révoquée ?)"),
            );
        }
    }
    for m in machines.iter().filter(|m| r.machines.is_empty() || r.machines.contains(&m.id) || r.toujours_en_ligne.contains(&m.id)) {
        let nom = m.nom.clone().unwrap_or_else(|| m.id.clone());
        let ligne = Mesure::new(format!("ricochet:{}", m.id), "Machine", nom.as_str());
        if !m.connectee {
            let depuis = m.vue_le.map(|v| format!("vue il y a {}", duree(t - v))).unwrap_or_else(|| "jamais vue".into());
            let critique = r.toujours_en_ligne.contains(&m.id);
            let ligne = ligne.valeur("hors ligne").detail(depuis);
            mesures.push(if critique { ligne.niveau(Niveau::Critique) } else { ligne });
            continue;
        }
        mesures.push(ligne.valeur("en ligne"));
        // État trop ancien (agent bloqué) : on ne juge pas sur de vieilles valeurs.
        let Some(e) = m.etat.as_ref().filter(|_| m.etat_recu_le.is_some_and(|r| t - r < 600)) else { continue };
        if e.memoire_totale > 0 {
            let p = e.memoire_utilisee as f64 * 100.0 / e.memoire_totale as f64;
            mesures.push(
                Mesure::new(format!("ricochet:{}:memoire", m.id), "Mémoire", nom.as_str())
                    .niveau(Niveau::par_seuils(p, c.memoire.attention, c.memoire.critique))
                    .valeur(format!("{} %", nombre(p, 0)))
                    .detail(format!("{} utilisés sur {}", octets(e.memoire_utilisee), octets(e.memoire_totale)))
                    .metrique(p),
            );
        }
        for d in e.disques.iter().filter(|d| d.total > 0) {
            let p = d.utilise as f64 * 100.0 / d.total as f64;
            mesures.push(
                Mesure::new(format!("ricochet:{}:disque:{}", m.id, d.point_montage), "Disque", format!("{nom} {}", d.point_montage))
                    .niveau(Niveau::par_seuils(p, c.disque.attention, c.disque.critique))
                    .valeur(format!("{} %", nombre(p, 0)))
                    .detail(format!("{} libres", octets(d.total.saturating_sub(d.utilise))))
                    .metrique(p),
            );
        }
    }
    mesures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machines() {
        let json = r#"[
          {"id":"pc","nom":"PC","connectee":false,"connectee_le":null,"vue_le":1000,"adresse":null,"info":null,"etat":null,"etat_recu_le":null},
          {"id":"srv","nom":null,"connectee":true,"vue_le":4000,"etat_recu_le":3900,"etat":{"cpu_pourcent":3.0,"nombre_coeurs":2,"charge":[0,0,0],
            "memoire_utilisee":50,"memoire_totale":100,"allumee_depuis_s":5,
            "disques":[{"point_montage":"/","utilise":96,"total":100}]}}]"#;
        let l: Vec<VueMachine> = serde_json::from_str(json).unwrap();
        let c = Config::default();
        let r = Ricochet { relais: "x".into(), jeton: "j".into(), machines: vec![], toujours_en_ligne: vec!["srv".into(), "nas".into()] };
        let m = evaluer(&c, &r, &l, 4000);
        assert_eq!(m[0].valeur, "inconnue"); // nas
        assert_eq!((m[1].nom.as_str(), m[1].niveau), ("PC", Niveau::Ok)); // PC éteint : normal
        assert_eq!(m[1].detail, "vue il y a 50 min");
        assert_eq!(m[2].valeur, "en ligne");
        assert_eq!(m[3].metrique, Some(50.0));
        assert_eq!((m[4].nom.as_str(), m[4].niveau), ("srv /", Niveau::Critique));
        // Une heure plus tard, l'état de srv est trop vieux : seules restent les lignes.
        let m = evaluer(&c, &r, &l, 4000 + 3600);
        assert_eq!(m.len(), 3);
    }
}
