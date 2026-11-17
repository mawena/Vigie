//! Tableau de bord web léger : une page lisible sur téléphone, une API JSON.
//! Un seul fil, HTTP sans TLS (nginx devant), mot de passe en HTTP Basic, ou mode Portier
//! (en-tête `X-Portier-Utilisateur` posé par nginx, cru seulement depuis 127.0.0.1/::1).

use crate::config::{Portier, Web};
use crate::historique::{Historique, jours_avant_plein};
use crate::modele::{Mesure, Niveau, maintenant, nombre};
use sha2::{Digest, Sha256};
use std::net::{IpAddr, SocketAddr};
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
pub fn lancer(c: &Web, p: &Portier, nom: String, fichier: PathBuf, partage: Partage) -> Result<(), String> {
    let adresse: SocketAddr = c.adresse.parse().map_err(|e| format!("web.adresse « {} » : {e}", c.adresse))?;
    if c.mot_de_passe.is_empty() && !adresse.ip().is_loopback() {
        return Err("web : sans mot de passe, le tableau de bord n'écoute que sur 127.0.0.1".into());
    }
    let serveur = Server::http(adresse).map_err(|e| format!("web {adresse} : {e}"))?;
    let historique = Historique::ouvrir(&fichier)?;
    let acces = Acces {
        utilisateur: c.utilisateur.clone(),
        empreinte: c.mot_de_passe.to_lowercase(),
        portier: p.actif.then(|| ModePortier {
            utilisateurs: p.utilisateurs.iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect(),
            url: p.url.trim_end_matches('/').to_string(),
            deconnexion: p.adresse_deconnexion(),
        }),
    };
    if p.actif {
        eprintln!("mode Portier actif ({} utilisateur(s) autorisé(s))", p.utilisateurs.len());
    }
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
    /// Mode Portier, si `[portier] actif = true`.
    portier: Option<ModePortier>,
}

struct ModePortier {
    utilisateurs: Vec<String>,
    /// Adresse publique de Portier sans « / » final (vide : pas de lien de connexion).
    url: String,
    deconnexion: Option<String>,
}

const ENTETE_PORTIER: &str = "X-Portier-Utilisateur";

impl Acces {
    /// Mode Portier : option active, connexion venue de 127.0.0.1/::1 (nginx local), un seul
    /// en-tête `X-Portier-Utilisateur` dont la valeur est dans la liste des utilisateurs.
    fn autorise_portier(&self, ip: Option<IpAddr>, valeurs: &[&str]) -> bool {
        let Some(p) = &self.portier else { return false };
        let locale = match ip {
            Some(IpAddr::V4(v4)) => v4 == std::net::Ipv4Addr::LOCALHOST,
            Some(IpAddr::V6(v6)) => v6.is_loopback() || v6.to_ipv4_mapped() == Some(std::net::Ipv4Addr::LOCALHOST),
            None => false,
        };
        let [nom] = valeurs else { return false };
        let nom = nom.trim();
        locale && !nom.is_empty() && p.utilisateurs.iter().any(|u| u == nom)
    }

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
    let valeur = |nom: &'static str| req.headers().iter().find(|x| x.field.equiv(nom)).map(|x| x.value.as_str().to_string());
    let auth = valeur("Authorization");
    let portier: Vec<&str> = req.headers().iter().filter(|x| x.field.equiv(ENTETE_PORTIER)).map(|x| x.value.as_str()).collect();
    let par_portier = acces.autorise_portier(req.remote_addr().map(|a| a.ip()), &portier);
    let (code, type_, corps) = if !par_portier && !acces.autorise(auth.as_deref()) {
        let mut r = Response::from_string("Identification requise")
            .with_status_code(401)
            .with_header(entete("WWW-Authenticate", "Basic realm=\"Vigie\", charset=\"UTF-8\""));
        if let Some(lien) = acces.portier.as_ref().and_then(|p| lien_connexion(p, valeur("Host").as_deref(), valeur("X-Forwarded-Proto").as_deref(), &url)) {
            r = Response::from_string(page_401(&lien))
                .with_status_code(401)
                .with_header(entete("WWW-Authenticate", "Basic realm=\"Vigie\", charset=\"UTF-8\""))
                .with_header(entete("Content-Type", "text/html; charset=utf-8"))
                .with_header(entete("Cache-Control", "no-store"))
                .with_header(entete("Content-Security-Policy", "default-src 'none'; frame-ancestors 'none'"));
        }
        let _ = req.respond(r);
        return;
    } else {
        match (req.method().as_str(), chemin) {
            ("GET", "/") => {
                let jours = parametre(requete, "jours").and_then(|j| j.parse::<i64>().ok()).filter(|j| [1, 7, 30].contains(j)).unwrap_or(1);
                let etat = partage.lock().map(|e| (e.t, e.mesures.clone())).unwrap_or_default();
                let deconnexion = if par_portier { acces.portier.as_ref().and_then(|p| p.deconnexion.as_deref()) } else { None };
                (200, "text/html; charset=utf-8", page(nom, etat.0, &etat.1, h, jours, deconnexion))
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

/// Lien « Se connecter via Portier » : `<url>/connexion?retour=<adresse demandée, encodée>`.
/// L'adresse demandée est reconstruite depuis Host et X-Forwarded-Proto (posés par nginx) ;
/// Portier ne suit de toute façon que les retours de son domaine.
fn lien_connexion(p: &ModePortier, hote: Option<&str>, proto: Option<&str>, chemin: &str) -> Option<String> {
    if p.url.is_empty() {
        return None;
    }
    let hote = hote.map(str::trim).filter(|h| !h.is_empty() && !h.contains(['/', '\\', '@', ' ']))?;
    let proto = if proto.map(str::trim) == Some("http") { "http" } else { "https" };
    let chemin = if chemin.starts_with('/') { chemin } else { "/" };
    Some(format!("{}/connexion?retour={}", p.url, encoder(&format!("{proto}://{hote}{chemin}"))))
}

/// Encodage en pourcentage (tout sauf les caractères non réservés).
fn encoder(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Page de la réponse 401 en mode Portier : HTTP Basic reste possible, lien vers Portier.
fn page_401(lien: &str) -> String {
    format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Identification requise — Vigie</title></head>\
<body style=\"font-family:system-ui,sans-serif;max-width:32rem;margin:3rem auto;padding:0 1rem;line-height:1.5\">\
<h1>Identification requise</h1><p><a href=\"{}\" style=\"display:inline-block;padding:.6rem 1rem;border-radius:.4rem;background:#1f5fbf;color:#fff;text-decoration:none\">Se connecter via Portier</a></p></body></html>",
        echapper(lien)
    )
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

fn page(nom: &str, t: i64, mesures: &[Mesure], h: &Historique, jours: i64, deconnexion: Option<&str>) -> String {
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
        "</section><footer class=\"pied\">Vigie {} — <a href=\"/api/etat\">état en JSON</a>{}</footer></main></body></html>",
        env!("CARGO_PKG_VERSION"),
        deconnexion.map(|d| format!(" — <a href=\"{}\">Se déconnecter</a>", echapper(d))).unwrap_or_default()
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
        let a = Acces { utilisateur: "charles".into(), empreinte: empreinte("secret"), portier: None };
        use base64::Engine;
        let b = |s: &str| format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(s));
        assert!(a.autorise(Some(&b("charles:secret"))));
        assert!(!a.autorise(Some(&b("charles:faux"))));
        assert!(!a.autorise(Some(&b("autre:secret"))));
        assert!(!a.autorise(None));
        assert!(Acces { utilisateur: "x".into(), empreinte: String::new(), portier: None }.autorise(None));
    }

    fn acces_portier(actif: bool) -> Acces {
        Acces {
            utilisateur: "charles".into(),
            empreinte: empreinte("secret"),
            portier: actif.then(|| ModePortier {
                utilisateurs: vec!["charles-gamligo".into()],
                url: "https://portier.mawena.cloud".into(),
                deconnexion: Some("https://portier.mawena.cloud/".into()),
            }),
        }
    }

    #[test]
    fn portier_entete_accepte() {
        let a = acces_portier(true);
        assert!(a.autorise_portier(Some("127.0.0.1".parse().unwrap()), &["charles-gamligo"]));
        assert!(a.autorise_portier(Some("::1".parse().unwrap()), &["charles-gamligo"]));
        assert!(a.autorise_portier(Some("::ffff:127.0.0.1".parse().unwrap()), &["charles-gamligo"]));
    }

    #[test]
    fn portier_nom_hors_liste() {
        let a = acces_portier(true);
        let ip = Some("127.0.0.1".parse().unwrap());
        assert!(!a.autorise_portier(ip, &["intrus"]));
        assert!(!a.autorise_portier(ip, &[""]));
        assert!(!a.autorise_portier(ip, &[]));
        // Deux en-têtes : refusé, même si l'un est bon.
        assert!(!a.autorise_portier(ip, &["charles-gamligo", "intrus"]));
    }

    #[test]
    fn portier_ip_non_locale() {
        let a = acces_portier(true);
        for ip in ["192.0.2.10", "10.0.0.1", "127.0.0.2", "2001:db8::1"] {
            assert!(!a.autorise_portier(Some(ip.parse().unwrap()), &["charles-gamligo"]), "{ip}");
        }
        assert!(!a.autorise_portier(None, &["charles-gamligo"]));
    }

    #[test]
    fn portier_desactive() {
        let a = acces_portier(false);
        assert!(!a.autorise_portier(Some("127.0.0.1".parse().unwrap()), &["charles-gamligo"]));
        // HTTP Basic inchangé.
        use base64::Engine;
        assert!(a.autorise(Some(&format!("Basic {}", base64::engine::general_purpose::STANDARD.encode("charles:secret")))));
        assert!(!a.autorise(None));
    }

    #[test]
    fn portier_lien_connexion() {
        let a = acces_portier(true);
        let p = a.portier.as_ref().unwrap();
        assert_eq!(
            lien_connexion(p, Some("vigie.mawena.cloud"), Some("https"), "/?jours=7").unwrap(),
            "https://portier.mawena.cloud/connexion?retour=https%3A%2F%2Fvigie.mawena.cloud%2F%3Fjours%3D7"
        );
        assert!(lien_connexion(p, None, None, "/").is_none());
        assert!(lien_connexion(p, Some("x@y"), None, "/").is_none());
        let sans_url = ModePortier { utilisateurs: vec![], url: String::new(), deconnexion: None };
        assert!(lien_connexion(&sans_url, Some("vigie.mawena.cloud"), None, "/").is_none());
        assert!(page_401("https://p/connexion?retour=a&b").contains("Se connecter via Portier") );
        assert!(page_401("x&y").contains("x&amp;y"));
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
        let p = page("vps", 1, &m, &h, 1, None);
        assert!(p.contains("Critique") && p.contains("&lt;x&gt;") && !p.contains("<x>"));
        assert!(!p.contains("Se déconnecter"));
        let p = page("vps", 1, &m, &h, 1, Some("https://portier.mawena.cloud/"));
        assert!(p.contains("<a href=\"https://portier.mawena.cloud/\">Se déconnecter</a>"));
        let c = courbe(&[(0, 10.0), (60, 20.0)], true, "disque");
        assert!(c.contains("<path class=\"ligne\""));
        drop(h);
        for s in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{s}", f.display()));
        }
    }
}
