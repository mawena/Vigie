//! Tableau de bord web léger : une page lisible sur téléphone, une API JSON.
//! Un seul fil, HTTP sans TLS (nginx devant), mot de passe en HTTP Basic.

use crate::config::Web;
use crate::historique::{Historique, jours_avant_plein};
use crate::modele::{Mesure, Niveau, maintenant, nombre};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Request, Response, Server};

/// Dernière tournée, partagée entre la boucle de surveillance et le serveur web.
#[derive(Default)]
pub struct Etat {
    pub t: i64,
    pub mesures: Vec<Mesure>,
}

pub type Partage = Arc<Mutex<Etat>>;

const STYLE: &str = include_str!("web/style.css");
const SCRIPT: &str = include_str!("web/app.js");
const ICONE: &str = include_str!("../site/icone.svg");

/// Démarre le serveur dans un fil. Refuse d'écouter hors de 127.0.0.1 sans mot de passe.
pub fn lancer(c: &Web, nom: String, fichier: PathBuf, partage: Partage) -> Result<(), String> {
    let adresse: SocketAddr = c.adresse.parse().map_err(|e| format!("web.adresse « {} » : {e}", c.adresse))?;
    if c.mot_de_passe.is_empty() && !adresse.ip().is_loopback() {
        return Err("web : sans mot de passe, le tableau de bord n'écoute que sur 127.0.0.1".into());
    }
    let serveur = Server::http(adresse).map_err(|e| format!("web {adresse} : {e}"))?;
    let historique = Historique::ouvrir(&fichier)?;
    let acces = Acces { utilisateur: c.utilisateur.clone(), empreinte: c.mot_de_passe.to_lowercase() };
    eprintln!("tableau de bord sur http://{adresse}");
    std::thread::Builder::new()
        .name("web".into())
        .spawn(move || {
            for req in serveur.incoming_requests() {
                repondre(req, &acces, &nom, &historique, &partage);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

struct Acces {
    utilisateur: String,
    empreinte: String,
}

impl Acces {
    fn autorise(&self, entete: Option<&str>) -> bool {
        if self.empreinte.is_empty() {
            return true;
        }
        use base64::Engine;
        let Some(b) = entete.and_then(|e| e.strip_prefix("Basic ")) else { return false };
        let Ok(brut) = base64::engine::general_purpose::STANDARD.decode(b.trim()) else { return false };
        let texte = String::from_utf8_lossy(&brut);
        let Some((u, m)) = texte.split_once(':') else { return false };
        let empreinte = empreinte(m);
        // Comparaison en temps constant.
        let egal = empreinte.len() == self.empreinte.len()
            && empreinte.bytes().zip(self.empreinte.bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0;
        egal && u == self.utilisateur
    }
}

/// Empreinte SHA-256 (hexadécimal) d'un mot de passe.
pub fn empreinte(mot_de_passe: &str) -> String {
    hex::encode(Sha256::digest(mot_de_passe.as_bytes()))
}

fn entete(nom: &str, valeur: &str) -> Header {
    Header::from_bytes(nom.as_bytes(), valeur.as_bytes()).expect("en-tête valide")
}

fn repondre(req: Request, acces: &Acces, nom: &str, h: &Historique, partage: &Partage) {
    let url = req.url().to_string();
    let (chemin, requete) = url.split_once('?').unwrap_or((&url, ""));
    let auth = req.headers().iter().find(|x| x.field.equiv("Authorization")).map(|x| x.value.as_str().to_string());
    let (code, type_, corps) = if !acces.autorise(auth.as_deref()) {
        let r = Response::from_string("Identification requise")
            .with_status_code(401)
            .with_header(entete("WWW-Authenticate", "Basic realm=\"Vigie\", charset=\"UTF-8\""));
        let _ = req.respond(r);
        return;
    } else {
        match (req.method().as_str(), chemin) {
            ("GET", "/") => {
                let jours = parametre(requete, "jours").and_then(|j| j.parse::<i64>().ok()).filter(|j| [1, 7, 30].contains(j)).unwrap_or(1);
                let etat = partage.lock().map(|e| (e.t, e.mesures.clone())).unwrap_or_default();
                (200, "text/html; charset=utf-8", page(nom, etat.0, &etat.1, h, jours))
            }
            ("GET", "/api/etat") => {
                let e = partage.lock().map(|e| (e.t, e.mesures.clone())).unwrap_or_default();
                let json = serde_json::json!({ "machine": nom, "t": e.0, "mesures": e.1 });
                (200, "application/json", json.to_string())
            }
            ("GET", "/api/serie") => match parametre(requete, "cle") {
                Some(cle) => {
                    let jours = parametre(requete, "jours").and_then(|j| j.parse::<i64>().ok()).unwrap_or(1).clamp(1, 366);
                    let points = h.serie(&cle, maintenant() - jours * 86400);
                    (200, "application/json", serde_json::to_string(&points).unwrap_or_default())
                }
                None => (400, "text/plain; charset=utf-8", "paramètre cle manquant".into()),
            },
            ("GET", "/style.css") => (200, "text/css; charset=utf-8", STYLE.into()),
            ("GET", "/app.js") => (200, "text/javascript; charset=utf-8", SCRIPT.into()),
            ("GET", "/icone.svg") => (200, "image/svg+xml", ICONE.into()),
            ("GET", _) => (404, "text/plain; charset=utf-8", "introuvable".into()),
            _ => (405, "text/plain; charset=utf-8", "méthode refusée".into()),
        }
    };
    let r = Response::from_string(corps)
        .with_status_code(code)
        .with_header(entete("Content-Type", type_))
        .with_header(entete("Cache-Control", "no-store"))
        .with_header(entete("X-Content-Type-Options", "nosniff"))
        .with_header(entete("Referrer-Policy", "no-referrer"))
        .with_header(entete("Content-Security-Policy", "default-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"));
    let _ = req.respond(r);
}

/// Valeur d'un paramètre de requête (décodage %XX et +).
fn parametre(requete: &str, nom: &str) -> Option<String> {
    requete.split('&').find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k == nom).then(|| decoder(v))
    })
}

fn decoder(s: &str) -> String {
    let b = s.as_bytes();
    let mut sortie = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => sortie.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(v) => {
                        sortie.push(v);
                        i += 2;
                    }
                    Err(_) => sortie.push(b'%'),
                }
            }
            c => sortie.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&sortie).into_owned()
}

fn echapper(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn marque(n: Niveau) -> (&'static str, &'static str, &'static str) {
    // (classe, icône, libellé) : la couleur ne porte jamais seule le sens.
    match n {
        Niveau::Ok => ("ok", "✓", "OK"),
        Niveau::Attention => ("attention", "!", "Attention"),
        Niveau::Critique => ("critique", "✕", "Critique"),
    }
}

fn page(nom: &str, t: i64, mesures: &[Mesure], h: &Historique, jours: i64) -> String {
    let global = mesures.iter().map(|m| m.niveau).max().unwrap_or(Niveau::Ok);
    let (classe, icone, libelle) = marque(global);
    let problemes: Vec<&Mesure> = {
        let mut p: Vec<&Mesure> = mesures.iter().filter(|m| m.niveau != Niveau::Ok).collect();
        p.sort_by(|a, b| b.niveau.cmp(&a.niveau));
        p
    };
    let mut s = String::with_capacity(32 * 1024);
    s.push_str(&format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{libelle} — Vigie {nom}</title><link rel=\"icon\" href=\"/icone.svg\"><link rel=\"stylesheet\" href=\"/style.css\">\
<script src=\"/app.js\" defer></script></head><body><main>\
<header class=\"entete\"><h1><img src=\"/icone.svg\" alt=\"\" width=\"28\" height=\"28\"> Vigie <span class=\"machine\">{nom}</span></h1>\
<p class=\"global {classe}\"><span class=\"icone\" aria-hidden=\"true\">{icone}</span> {libelle}</p>\
<p class=\"maj\">Dernière tournée : {}</p></header>",
        if t > 0 { format!("<time data-t=\"{t}\" data-relatif>{t}</time>") } else { "en cours…".into() },
        nom = echapper(nom),
    ));

    // Problèmes en premier.
    s.push_str("<section aria-labelledby=\"t-problemes\"><h2 id=\"t-problemes\">Problèmes</h2>");
    if problemes.is_empty() {
        s.push_str("<p class=\"calme\">Tout va bien.</p>");
    } else {
        s.push_str("<ul class=\"problemes\">");
        for m in &problemes {
            let (c, i, l) = marque(m.niveau);
            s.push_str(&format!(
                "<li class=\"{c}\"><span class=\"badge {c}\"><span aria-hidden=\"true\">{i}</span> {l}</span> <strong>{} {}</strong> : {}{}</li>",
                m.categorie,
                echapper(&m.nom),
                echapper(&m.valeur),
                if m.detail.is_empty() { String::new() } else { format!(" — {}", echapper(&m.detail)) }
            ));
        }
        s.push_str("</ul>");
    }
    s.push_str("</section>");

    // Tendances.
    s.push_str("<section aria-labelledby=\"t-tendances\"><div class=\"titre-section\"><h2 id=\"t-tendances\">Tendances</h2><nav class=\"periodes\" aria-label=\"Période\">");
    for (j, l) in [(1, "24 h"), (7, "7 jours"), (30, "30 jours")] {
        let courant = if j == jours { " aria-current=\"page\"" } else { "" };
        s.push_str(&format!("<a href=\"/?jours={j}\"{courant}>{l}</a>"));
    }
    s.push_str("</nav></div><div class=\"cartes\">");
    let depuis = maintenant() - jours * 86400;
    for m in mesures.iter().filter(|m| matches!(m.categorie, "Disque" | "Mémoire" | "Charge" | "Température") && m.metrique.is_some()) {
        let points = reduire(&h.serie(&m.cle, depuis), 240);
        let pourcent = m.categorie != "Charge" && m.categorie != "Température";
        let mut note = String::new();
        if m.categorie == "Disque" {
            let semaine = h.serie(&m.cle, maintenant() - 7 * 86400);
            note = match jours_avant_plein(&semaine) {
                Some(j) if j < 365.0 => format!("<p class=\"note\">À ce rythme, plein dans environ {} jours.</p>", nombre(j, 0)),
                Some(_) => "<p class=\"note\">Remplissage très lent.</p>".into(),
                None if semaine.len() >= 12 && semaine.last().map(|p| p.0).unwrap_or(0) - semaine[0].0 >= 86400 => {
                    "<p class=\"note\">Stable sur 7 jours.</p>".into()
                }
                None => "<p class=\"note\">Tendance disponible après un jour d'historique.</p>".into(),
            };
        }
        let (c, i, l) = marque(m.niveau);
        s.push_str(&format!(
            "<article class=\"carte\"><h3>{} {}</h3><p class=\"valeur\">{} <span class=\"badge {c}\"><span aria-hidden=\"true\">{i}</span> {l}</span></p>{}{note}</article>",
            m.categorie,
            echapper(&m.nom),
            echapper(&m.valeur),
            courbe(&points, pourcent, &m.categorie.to_lowercase()),
        ));
    }
    s.push_str("</div></section>");

    // Toutes les mesures, par catégorie : la vue tableau.
    s.push_str("<section aria-labelledby=\"t-mesures\"><h2 id=\"t-mesures\">Toutes les mesures</h2><div class=\"defile\"><table><thead><tr><th scope=\"col\">État</th><th scope=\"col\">Sonde</th><th scope=\"col\">Valeur</th><th scope=\"col\">Détail</th></tr></thead><tbody>");
    let mut triees: Vec<&Mesure> = mesures.iter().collect();
    triees.sort_by_key(|m| m.categorie);
    for m in triees {
        let (c, i, l) = marque(m.niveau);
        s.push_str(&format!(
            "<tr><td><span class=\"badge {c}\"><span aria-hidden=\"true\">{i}</span> {l}</span></td><td>{} <strong>{}</strong></td><td class=\"nombre\">{}</td><td>{}</td></tr>",
            m.categorie,
            echapper(&m.nom),
            echapper(&m.valeur),
            echapper(&m.detail)
        ));
    }
    s.push_str("</tbody></table></div></section>");

    // Journal des alertes.
    s.push_str("<section aria-labelledby=\"t-journal\"><h2 id=\"t-journal\">Dernières alertes</h2>");
    let evenements = h.evenements(30);
    if evenements.is_empty() {
        s.push_str("<p class=\"calme\">Aucune alerte enregistrée.</p>");
    } else {
        s.push_str("<ul class=\"journal\">");
        for (t, niveau, texte) in evenements {
            let (c, i, l) = match niveau.as_str() {
                "CRITIQUE" => marque(Niveau::Critique),
                "ATTENTION" => marque(Niveau::Attention),
                _ => ("ok", "✓", "Retour"),
            };
            s.push_str(&format!(
                "<li><time data-t=\"{t}\">{t}</time> <span class=\"badge {c}\"><span aria-hidden=\"true\">{i}</span> {l}</span> {}</li>",
                echapper(&texte)
            ));
        }
        s.push_str("</ul>");
    }
    s.push_str(&format!(
        "</section><footer class=\"pied\">Vigie {} — <a href=\"/api/etat\">état en JSON</a></footer></main></body></html>",
        env!("CARGO_PKG_VERSION")
    ));
    s
}

/// Réduit une série à `max` points au plus (moyenne par paquet), pour garder la page légère.
fn reduire(points: &[(i64, f64)], max: usize) -> Vec<(i64, f64)> {
    if points.len() <= max {
        return points.to_vec();
    }
    let taille = points.len().div_ceil(max);
    points
        .chunks(taille)
        .map(|p| (p[p.len() / 2].0, p.iter().map(|x| x.1).sum::<f64>() / p.len() as f64))
        .collect()
}

/// Courbe SVG d'une seule série : ligne de 2 px, grille discrète, dernier point marqué.
/// Les points sont aussi joints en JSON pour l'infobulle (app.js).
fn courbe(points: &[(i64, f64)], pourcent: bool, unite: &str) -> String {
    if points.len() < 2 {
        return "<p class=\"vide\">Pas encore assez d'historique pour tracer une courbe.</p>".into();
    }
    let (l, h, g, d, haut, bas) = (600.0, 150.0, 34.0, 8.0, 8.0, 20.0);
    let (t0, t1) = (points[0].0 as f64, points[points.len() - 1].0 as f64);
    let vmax = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    let ymax = if pourcent { 100.0 } else { echelle(vmax) };
    let x = |t: i64| g + (t as f64 - t0) / (t1 - t0).max(1.0) * (l - g - d);
    let y = |v: f64| haut + (1.0 - (v / ymax).clamp(0.0, 1.0)) * (h - haut - bas);
    let mut svg = format!(
        "<figure class=\"courbe\"><svg viewBox=\"0 0 {l} {h}\" role=\"img\" aria-label=\"Évolution, {} points\" data-points='{}' data-unite=\"{}\" data-g=\"{g}\" data-d=\"{d}\" data-haut=\"{haut}\" data-bas=\"{bas}\" data-ymax=\"{ymax}\">",
        points.len(),
        serde_json::to_string(points).unwrap_or_default(),
        if pourcent { "%" } else if unite == "température" { "°C" } else { "" },
    );
    for k in 0..=2 {
        let v = ymax * k as f64 / 2.0;
        let yy = y(v);
        svg.push_str(&format!(
            "<line class=\"grille\" x1=\"{g}\" x2=\"{}\" y1=\"{yy:.1}\" y2=\"{yy:.1}\"/><text class=\"axe\" x=\"{}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            l - d,
            g - 6.0,
            yy + 4.0,
            nombre(v, if ymax < 3.0 { 1 } else { 0 })
        ));
    }
    let chemin: Vec<String> = points
        .iter()
        .enumerate()
        .map(|(i, (t, v))| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, x(*t), y(*v)))
        .collect();
    let (tf, vf) = points[points.len() - 1];
    svg.push_str(&format!(
        "<path class=\"ligne\" d=\"{}\"/><circle class=\"dernier\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\"/>\
<text class=\"axe\" x=\"{g}\" y=\"{}\"><tspan data-t=\"{}\" data-court></tspan></text>\
<text class=\"axe\" x=\"{}\" y=\"{}\" text-anchor=\"end\"><tspan data-t=\"{tf}\" data-court></tspan></text></svg></figure>",
        chemin.join(""),
        x(tf),
        y(vf),
        h - 4.0,
        points[0].0,
        l - d,
        h - 4.0,
    ));
    svg
}

/// Haut d'axe « rond » au-dessus de la valeur maximale.
fn echelle(v: f64) -> f64 {
    for pas in [0.5, 1.0, 2.0, 3.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0] {
        if v <= pas * 0.95 {
            return pas;
        }
    }
    (v * 1.2).ceil()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mot_de_passe() {
        let a = Acces { utilisateur: "charles".into(), empreinte: empreinte("secret") };
        use base64::Engine;
        let b = |s: &str| format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(s));
        assert!(a.autorise(Some(&b("charles:secret"))));
        assert!(!a.autorise(Some(&b("charles:faux"))));
        assert!(!a.autorise(Some(&b("autre:secret"))));
        assert!(!a.autorise(None));
        assert!(Acces { utilisateur: "x".into(), empreinte: String::new() }.autorise(None));
    }

    #[test]
    fn utilitaires() {
        assert_eq!(parametre("cle=disque%3A%2F&jours=7", "cle").unwrap(), "disque:/");
        assert_eq!(parametre("a=1", "b"), None);
        assert_eq!(decoder("a+b%"), "a b%");
        assert_eq!(echapper("<a href='x'>&"), "&lt;a href=&#39;x&#39;&gt;&amp;");
        assert_eq!(reduire(&(0..1000).map(|i| (i, 1.0)).collect::<Vec<_>>(), 240).len(), 200);
        assert_eq!(echelle(0.4), 0.5);
        assert_eq!(echelle(1.2), 2.0);
    }

    #[test]
    fn page_complete() {
        let f = std::env::temp_dir().join(format!("vigie-web-{}.db", std::process::id()));
        let h = Historique::ouvrir(&f).unwrap();
        let m = vec![
            Mesure::new("disque:/", "Disque", "/").valeur("50 %").metrique(50.0),
            Mesure::new("http:<x>", "Web", "<x>").niveau(Niveau::Critique).valeur("injoignable"),
        ];
        let p = page("vps", 1, &m, &h, 1);
        assert!(p.contains("Critique") && p.contains("&lt;x&gt;") && !p.contains("<x>"));
        let c = courbe(&[(0, 10.0), (60, 20.0)], true, "disque");
        assert!(c.contains("<path class=\"ligne\""));
        drop(h);
        for s in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{s}", f.display()));
        }
    }
}
