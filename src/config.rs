//! Configuration TOML. Chaque section a des valeurs par défaut : un fichier vide suffit.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Nom de la machine dans les alertes.
    pub nom: String,
    /// Secondes entre deux tournées de sondes.
    pub intervalle: u64,
    pub disque: Disque,
    pub memoire: Seuils,
    pub charge: Seuils,
    pub temperature: Seuils,
    pub systemd: Systemd,
    pub docker: Docker,
    pub libvirt: Libvirt,
    pub http: Vec<Http>,
    pub tls: Vec<Tls>,
    pub dns: Vec<Dns>,
    pub sauvegarde: Vec<Sauvegarde>,
    pub alertes: Alertes,
    pub ntfy: Option<Ntfy>,
    pub telegram: Option<Telegram>,
    pub courriel: Option<Courriel>,
    pub historique: Historique,
    pub web: Web,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            nom: nom_machine(),
            intervalle: 60,
            disque: Disque::default(),
            memoire: Seuils { actif: true, attention: 90.0, critique: 97.0 },
            charge: Seuils { actif: true, attention: 1.5, critique: 3.0 },
            temperature: Seuils { actif: true, attention: 80.0, critique: 95.0 },
            systemd: Systemd::default(),
            docker: Docker::default(),
            libvirt: Libvirt::default(),
            http: Vec::new(),
            tls: Vec::new(),
            dns: Vec::new(),
            sauvegarde: Vec::new(),
            alertes: Alertes::default(),
            ntfy: None,
            telegram: None,
            courriel: None,
            historique: Historique::default(),
            web: Web::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Seuils {
    pub actif: bool,
    pub attention: f64,
    pub critique: f64,
}

impl Default for Seuils {
    fn default() -> Self {
        Seuils { actif: true, attention: 85.0, critique: 95.0 }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Disque {
    pub chemins: Vec<String>,
    pub attention: f64,
    pub critique: f64,
}

impl Default for Disque {
    fn default() -> Self {
        Disque { chemins: vec!["/".into()], attention: 85.0, critique: 95.0 }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Systemd {
    /// Alerter sur toute unité système en échec.
    pub echecs: bool,
    /// Unités en échec à ignorer.
    pub ignorer: Vec<String>,
    /// Unités qui doivent être actives.
    pub unites: Vec<String>,
    /// Unités utilisateur (systemctl --user) qui doivent être actives.
    pub unites_utilisateur: Vec<String>,
}

impl Default for Systemd {
    fn default() -> Self {
        Systemd { echecs: true, ignorer: Vec::new(), unites: Vec::new(), unites_utilisateur: Vec::new() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Docker {
    /// `auto` : actif si le socket existe.
    pub actif: Option<bool>,
    pub socket: String,
    /// Conteneurs qui doivent tourner.
    pub conteneurs: Vec<String>,
    /// Signaler tout conteneur `unhealthy` ou qui redémarre en boucle.
    pub signaler_malades: bool,
}

impl Default for Docker {
    fn default() -> Self {
        Docker { actif: None, socket: "/var/run/docker.sock".into(), conteneurs: Vec::new(), signaler_malades: true }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Libvirt {
    pub uri: String,
    /// Machines virtuelles qui doivent tourner.
    pub domaines: Vec<String>,
}

impl Default for Libvirt {
    fn default() -> Self {
        Libvirt { uri: "qemu:///system".into(), domaines: Vec::new() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Http {
    pub nom: String,
    pub url: String,
    /// Plus haut code de réponse accepté.
    #[serde(default = "statut_max")]
    pub statut_max: u16,
    /// Texte qui doit apparaître dans la réponse.
    #[serde(default)]
    pub contient: Option<String>,
    /// Délai maximal, en secondes.
    #[serde(default = "delai")]
    pub delai: u64,
    /// Au-delà (secondes), la réponse est jugée lente.
    #[serde(default = "lent")]
    pub lent: f64,
}

fn statut_max() -> u16 {
    399
}
fn delai() -> u64 {
    10
}
fn lent() -> f64 {
    5.0
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tls {
    pub hote: String,
    #[serde(default = "port_https")]
    pub port: u16,
    #[serde(default = "tls_attention")]
    pub attention_jours: f64,
    #[serde(default = "tls_critique")]
    pub critique_jours: f64,
}

fn port_https() -> u16 {
    443
}
fn tls_attention() -> f64 {
    14.0
}
fn tls_critique() -> f64 {
    5.0
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dns {
    pub nom: String,
    /// Adresses attendues (vide : il suffit que le nom résolve).
    #[serde(default)]
    pub attendu: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sauvegarde {
    pub nom: String,
    /// Fichier ou dossier : on regarde le fichier le plus récent (sous-dossiers compris).
    pub chemin: String,
    /// Âge maximal, en heures.
    pub max_heures: f64,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Alertes {
    /// Tournées consécutives avant d'alerter (et avant d'annoncer le retour à la normale).
    pub duree_min: u32,
    /// Minutes entre deux rappels d'un problème qui dure (0 : jamais).
    pub rappel: u64,
    /// Annoncer le retour à la normale.
    pub retour: bool,
    /// Alerter aussi pour le niveau « attention » (sinon seulement « critique »).
    pub attention: bool,
}

impl Default for Alertes {
    fn default() -> Self {
        Alertes { duree_min: 2, rappel: 360, retour: true, attention: true }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ntfy {
    #[serde(default = "ntfy_serveur")]
    pub serveur: String,
    pub sujet: String,
    #[serde(default)]
    pub jeton: Option<String>,
}

fn ntfy_serveur() -> String {
    "https://ntfy.sh".into()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Telegram {
    pub jeton: String,
    pub chat: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Courriel {
    pub a: String,
    #[serde(default = "courriel_de")]
    pub de: String,
    #[serde(default = "sendmail")]
    pub commande: String,
}

fn courriel_de() -> String {
    format!("vigie@{}", nom_machine())
}
fn sendmail() -> String {
    "/usr/sbin/sendmail".into()
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Historique {
    pub actif: bool,
    /// Secondes entre deux points enregistrés.
    pub pas: u64,
    pub garder_jours: u64,
    /// Vide : ~/.local/share/vigie/historique.db
    pub fichier: String,
}

impl Default for Historique {
    fn default() -> Self {
        Historique { actif: true, pas: 300, garder_jours: 30, fichier: String::new() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Web {
    pub actif: bool,
    pub adresse: String,
    pub utilisateur: String,
    /// Empreinte SHA-256 (hexadécimal) du mot de passe : `vigie mot-de-passe`.
    pub mot_de_passe: String,
}

impl Default for Web {
    fn default() -> Self {
        Web { actif: true, adresse: "127.0.0.1:8790".into(), utilisateur: "vigie".into(), mot_de_passe: String::new() }
    }
}

impl Config {
    pub fn lire(chemin: &Path) -> Result<Config, String> {
        let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        Config::depuis_texte(&texte).map_err(|e| format!("{} : {e}", chemin.display()))
    }

    pub fn depuis_texte(texte: &str) -> Result<Config, String> {
        let c: Config = toml::from_str(texte).map_err(|e| e.to_string())?;
        c.valider()?;
        Ok(c)
    }

    fn valider(&self) -> Result<(), String> {
        if self.intervalle < 10 {
            return Err("intervalle : 10 secondes au minimum".into());
        }
        if self.alertes.duree_min == 0 {
            return Err("alertes.duree_min : 1 au minimum".into());
        }
        for h in &self.http {
            if !h.url.starts_with("http://") && !h.url.starts_with("https://") {
                return Err(format!("http « {} » : l'adresse doit commencer par http:// ou https://", h.nom));
            }
        }
        if !self.web.mot_de_passe.is_empty() && (self.web.mot_de_passe.len() != 64 || hex::decode(&self.web.mot_de_passe).is_err()) {
            return Err("web.mot_de_passe : empreinte SHA-256 attendue (vigie mot-de-passe)".into());
        }
        Ok(())
    }

    /// Fichier de l'historique.
    pub fn fichier_historique(&self) -> PathBuf {
        if self.historique.fichier.is_empty() {
            dossier_donnees().join("historique.db")
        } else {
            PathBuf::from(&self.historique.fichier)
        }
    }
}

/// `$VIGIE_CONFIG`, sinon `~/.config/vigie/vigie.toml`.
pub fn chemin_par_defaut() -> PathBuf {
    if let Some(c) = std::env::var_os("VIGIE_CONFIG") {
        return PathBuf::from(c);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| maison().join(".config"));
    base.join("vigie").join("vigie.toml")
}

fn dossier_donnees() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| maison().join(".local/share"))
        .join("vigie")
}

fn maison() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn nom_machine() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "machine".into())
}

/// Configuration d'exemple, commentée (exemples/vigie.toml).
pub const EXEMPLE: &str = include_str!("../exemples/vigie.toml");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fichier_vide() {
        let c = Config::depuis_texte("").unwrap();
        assert_eq!(c.intervalle, 60);
        assert_eq!(c.disque.chemins, vec!["/"]);
        assert!(c.ntfy.is_none());
    }

    #[test]
    fn exemple_valide() {
        let c = Config::depuis_texte(EXEMPLE).unwrap();
        assert!(!c.http.is_empty());
        assert!(c.ntfy.is_some());
    }

    #[test]
    fn erreurs() {
        assert!(Config::depuis_texte("inconnu = 1").is_err());
        assert!(Config::depuis_texte("intervalle = 1").is_err());
        assert!(Config::depuis_texte("[[http]]\nnom = \"x\"\nurl = \"ftp://x\"").is_err());
        assert!(Config::depuis_texte("[web]\nmot_de_passe = \"abc\"").is_err());
    }
}
