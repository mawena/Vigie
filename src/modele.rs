//! Types communs : niveau d'une mesure et résultat d'une sonde.

use serde::Serialize;

/// Gravité d'une mesure, dans l'ordre croissant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Niveau {
    Ok,
    Attention,
    Critique,
}

impl Niveau {
    pub fn libelle(self) -> &'static str {
        match self {
            Niveau::Ok => "OK",
            Niveau::Attention => "ATTENTION",
            Niveau::Critique => "CRITIQUE",
        }
    }

    /// Niveau d'une valeur qui ne doit pas monter au-dessus des seuils.
    pub fn par_seuils(valeur: f64, attention: f64, critique: f64) -> Niveau {
        if valeur >= critique {
            Niveau::Critique
        } else if valeur >= attention {
            Niveau::Attention
        } else {
            Niveau::Ok
        }
    }
}

/// Résultat d'une vérification.
#[derive(Debug, Clone, Serialize)]
pub struct Mesure {
    /// Identifiant stable, par exemple `disque:/` ou `http:CCS`.
    pub cle: String,
    /// Catégorie affichée : Disque, Mémoire, Docker…
    pub categorie: &'static str,
    /// Nom lisible de l'objet surveillé.
    pub nom: String,
    pub niveau: Niveau,
    /// Valeur courte, par exemple « 62 % » ou « 200 en 0,31 s ».
    pub valeur: String,
    /// Explication en cas de problème.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// Valeur numérique gardée dans l'historique.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrique: Option<f64>,
}

impl Mesure {
    pub fn new(cle: impl Into<String>, categorie: &'static str, nom: impl Into<String>) -> Self {
        Mesure {
            cle: cle.into(),
            categorie,
            nom: nom.into(),
            niveau: Niveau::Ok,
            valeur: String::new(),
            detail: String::new(),
            metrique: None,
        }
    }

    pub fn niveau(mut self, niveau: Niveau) -> Self {
        self.niveau = niveau;
        self
    }

    pub fn valeur(mut self, valeur: impl Into<String>) -> Self {
        self.valeur = valeur.into();
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn metrique(mut self, metrique: f64) -> Self {
        self.metrique = Some(metrique);
        self
    }

    /// Sonde qui n'a pas pu s'exécuter : on le signale en « attention ».
    pub fn erreur(cle: impl Into<String>, categorie: &'static str, nom: impl Into<String>, e: impl std::fmt::Display) -> Self {
        Mesure::new(cle, categorie, nom)
            .niveau(Niveau::Attention)
            .valeur("erreur")
            .detail(format!("sonde impossible : {e}"))
    }
}

/// Nombre à la française : virgule décimale.
pub fn nombre(v: f64, decimales: usize) -> String {
    format!("{v:.decimales$}").replace('.', ",")
}

/// Secondes écoulées depuis l'époque Unix.
pub fn maintenant() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Durée lisible : « 3 j 4 h », « 2 h 05 », « 12 min ».
pub fn duree(secondes: i64) -> String {
    let s = secondes.max(0);
    let (j, h, m) = (s / 86400, (s % 86400) / 3600, (s % 3600) / 60);
    if j > 0 {
        format!("{j} j {h} h")
    } else if h > 0 {
        format!("{h} h {m:02}")
    } else {
        format!("{m} min")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seuils() {
        assert_eq!(Niveau::par_seuils(50.0, 85.0, 95.0), Niveau::Ok);
        assert_eq!(Niveau::par_seuils(85.0, 85.0, 95.0), Niveau::Attention);
        assert_eq!(Niveau::par_seuils(99.0, 85.0, 95.0), Niveau::Critique);
        assert!(Niveau::Critique > Niveau::Attention && Niveau::Attention > Niveau::Ok);
    }

    #[test]
    fn formats() {
        assert_eq!(nombre(1.256, 2), "1,26");
        assert_eq!(duree(90), "1 min");
        assert_eq!(duree(2 * 3600 + 5 * 60), "2 h 05");
        assert_eq!(duree(3 * 86400 + 4 * 3600), "3 j 4 h");
    }
}
