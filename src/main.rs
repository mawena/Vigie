//! Vigie : surveillance légère d'un serveur, alertes sur le téléphone.

mod alertes;
mod config;
mod historique;
mod modele;
mod sondes;
mod web;

use config::Config;
use modele::{Mesure, Niveau};
use std::path::PathBuf;
use std::process::ExitCode;

const AIDE: &str = "Vigie — surveillance légère, alertes sur le téléphone

Usage : vigie [--config FICHIER] <commande>

Commandes :
  etat [--json]     lance toutes les sondes une fois et affiche le résultat
  surveiller        surveille en continu et envoie les alertes (service systemd)
  test-alerte       envoie un message d'essai sur tous les canaux configurés
  mot-de-passe      lit un mot de passe sur l'entrée et affiche son empreinte (section [web])
  config-exemple    affiche une configuration commentée
  version           affiche la version

Configuration : --config, sinon $VIGIE_CONFIG, sinon ~/.config/vigie/vigie.toml
(absente : valeurs par défaut).";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut chemin: Option<PathBuf> = None;
    if let Some(i) = args.iter().position(|a| a == "--config" || a == "-c") {
        if i + 1 >= args.len() {
            eprintln!("--config : fichier manquant");
            return ExitCode::from(2);
        }
        chemin = Some(PathBuf::from(args.remove(i + 1)));
        args.remove(i);
    }
    let commande = args.first().map(String::as_str).unwrap_or("aide");
    let options = &args[args.len().min(1)..];

    match commande {
        "version" | "--version" | "-V" => {
            println!("vigie {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "aide" | "help" | "--help" | "-h" => {
            println!("{AIDE}");
            ExitCode::SUCCESS
        }
        "config-exemple" => {
            print!("{}", config::EXEMPLE);
            ExitCode::SUCCESS
        }
        "etat" => match charger(chemin) {
            Ok(c) => etat(&c, options.iter().any(|o| o == "--json")),
            Err(e) => erreur(e),
        },
        "surveiller" => match charger(chemin) {
            Ok(c) => surveiller(c),
            Err(e) => erreur(e),
        },
        "test-alerte" => match charger(chemin) {
            Ok(c) => test_alerte(&c),
            Err(e) => erreur(e),
        },
        "mot-de-passe" => mot_de_passe(),
        autre => {
            eprintln!("commande inconnue : {autre}\n\n{AIDE}");
            ExitCode::from(2)
        }
    }
}

fn erreur(e: String) -> ExitCode {
    eprintln!("vigie : {e}");
    ExitCode::from(1)
}

/// Lit la configuration ; un fichier par défaut absent donne les valeurs par défaut.
fn charger(chemin: Option<PathBuf>) -> Result<Config, String> {
    match chemin {
        Some(c) => Config::lire(&c),
        None => {
            let c = config::chemin_par_defaut();
            if c.exists() { Config::lire(&c) } else { Ok(Config::default()) }
        }
    }
}

/// `vigie etat` : code de sortie 0 si tout va bien, 1 si attention, 2 si critique.
fn etat(c: &Config, json: bool) -> ExitCode {
    let mesures = sondes::tournee(c);
    if json {
        println!("{}", serde_json::to_string_pretty(&mesures).unwrap_or_default());
    } else {
        afficher(&c.nom, &mesures);
    }
    match mesures.iter().map(|m| m.niveau).max().unwrap_or(Niveau::Ok) {
        Niveau::Ok => ExitCode::SUCCESS,
        Niveau::Attention => ExitCode::from(1),
        Niveau::Critique => ExitCode::from(2),
    }
}

fn afficher(nom: &str, mesures: &[Mesure]) {
    use std::io::IsTerminal;
    let couleur = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let largeur = mesures.iter().map(|m| m.categorie.chars().count() + m.nom.chars().count() + 1).max().unwrap_or(10);
    println!("Vigie — {nom}");
    for m in mesures {
        let (debut, fin) = match (couleur, m.niveau) {
            (false, _) | (true, Niveau::Ok) => ("", ""),
            (true, Niveau::Attention) => ("\x1b[33m", "\x1b[0m"),
            (true, Niveau::Critique) => ("\x1b[31;1m", "\x1b[0m"),
        };
        let objet = format!("{} {}", m.categorie, m.nom);
        let pad = largeur.saturating_sub(objet.chars().count());
        let marque = match m.niveau {
            Niveau::Ok => "✓",
            Niveau::Attention => "!",
            Niveau::Critique => "✗",
        };
        print!("{debut}{marque} {objet}{} {}{fin}", " ".repeat(pad), m.valeur);
        if !m.detail.is_empty() {
            print!("  — {}", m.detail);
        }
        println!();
    }
    let (a, c) = (
        mesures.iter().filter(|m| m.niveau == Niveau::Attention).count(),
        mesures.iter().filter(|m| m.niveau == Niveau::Critique).count(),
    );
    println!("{} mesures : {} critique(s), {} attention(s)", mesures.len(), c, a);
}

fn canaux(c: &Config) -> usize {
    [c.ntfy.is_some(), c.telegram.is_some(), c.courriel.is_some()].iter().filter(|x| **x).count()
}

fn test_alerte(c: &Config) -> ExitCode {
    if canaux(c) == 0 {
        return erreur("aucun canal d'alerte configuré ([ntfy], [telegram] ou [courriel])".into());
    }
    let m = alertes::Message {
        titre: format!("{} : essai", c.nom),
        corps: "Message d'essai de Vigie : les alertes arrivent bien.".into(),
        niveau: Niveau::Attention,
    };
    match alertes::envoyer(c, &m) {
        Ok(n) => {
            println!("Message d'essai envoyé sur {n} canal(aux).");
            ExitCode::SUCCESS
        }
        Err(es) => erreur(es.join("\n")),
    }
}

/// Boucle principale : une tournée par intervalle, alertes, messages en attente réessayés.
fn surveiller(c: Config) -> ExitCode {
    eprintln!("vigie {} : surveillance de {} toutes les {} s", env!("CARGO_PKG_VERSION"), c.nom, c.intervalle);
    if canaux(&c) == 0 {
        eprintln!("attention : aucun canal d'alerte configuré, les problèmes ne seront que journalisés");
    }
    let partage: web::Partage = Default::default();
    let mut historique = if c.historique.actif {
        match historique::Historique::ouvrir(&c.fichier_historique()) {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("historique désactivé : {e}");
                None
            }
        }
    } else {
        None
    };
    if c.web.actif {
        if historique.is_none() {
            eprintln!("tableau de bord désactivé : il lit l'historique");
        } else if let Err(e) = web::lancer(&c.web, c.nom.clone(), c.fichier_historique(), partage.clone()) {
            eprintln!("tableau de bord désactivé : {e}");
        }
    }
    let mut veilleur = alertes::Veilleur::default();
    let mut en_attente: Vec<(alertes::Message, u32)> = Vec::new();
    loop {
        let debut = std::time::Instant::now();
        let mesures = sondes::tournee(&c);
        let t = modele::maintenant();
        let evenements = veilleur.evaluer(&c.alertes, &mesures, t);
        for e in &evenements {
            eprintln!("{} {}", if e.retour { "RETOUR" } else { e.niveau.libelle() }, e.texte);
        }
        if let Some(h) = historique.as_mut() {
            if let Err(e) = h.enregistrer(&mesures, t, c.historique.pas, c.historique.garder_jours).and_then(|_| h.noter(&evenements, t)) {
                eprintln!("historique : {e}");
            }
        }
        if canaux(&c) > 0 {
            if let Some(m) = alertes::composer(&c.nom, &evenements) {
                en_attente.push((m, 0));
            }
            // Un message qui n'est parti sur aucun canal est réessayé aux tournées suivantes (10 fois).
            let mut reste = Vec::new();
            for (m, essais) in en_attente.drain(..) {
                match alertes::envoyer(&c, &m) {
                    Ok(_) => {}
                    Err(es) => {
                        for e in &es {
                            eprintln!("envoi impossible : {e}");
                        }
                        if es.len() == canaux(&c) && essais < 10 {
                            reste.push((m, essais + 1));
                        }
                    }
                }
            }
            en_attente = reste;
        }
        if let Ok(mut e) = partage.lock() {
            e.t = t;
            e.mesures = mesures;
        }
        let pause = std::time::Duration::from_secs(c.intervalle).saturating_sub(debut.elapsed());
        std::thread::sleep(pause);
    }
}

fn mot_de_passe() -> ExitCode {
    use std::io::{BufRead, IsTerminal};
    if std::io::stdin().is_terminal() {
        eprintln!("Mot de passe (affiché en clair, puis Entrée) :");
    }
    let mut ligne = String::new();
    if std::io::stdin().lock().read_line(&mut ligne).is_err() || ligne.trim_end_matches(['\r', '\n']).is_empty() {
        return erreur("mot de passe vide".into());
    }
    println!("{}", web::empreinte(ligne.trim_end_matches(['\r', '\n'])));
    ExitCode::SUCCESS
}
