//! Vigie : surveillance légère d'un serveur, alertes sur le téléphone.

mod config;
mod modele;
mod sondes;

use config::Config;
use modele::{Mesure, Niveau};
use std::path::PathBuf;
use std::process::ExitCode;

const AIDE: &str = "Vigie — surveillance légère, alertes sur le téléphone

Usage : vigie [--config FICHIER] <commande>

Commandes :
  etat [--json]     lance toutes les sondes une fois et affiche le résultat
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
