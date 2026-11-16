//! Sondes réseau : sites HTTP(S), certificats TLS, résolution DNS.

use crate::config::{Dns, Http, Tls};
use crate::modele::{Mesure, Niveau, maintenant, nombre};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub fn http(c: &Http) -> Mesure {
    let cle = format!("http:{}", c.nom);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(c.delai)))
        .http_status_as_error(false)
        .user_agent(concat!("vigie/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let debut = Instant::now();
    let mut reponse = match agent.get(&c.url).call() {
        Ok(r) => r,
        Err(e) => {
            return Mesure::new(cle, "Web", c.nom.as_str()).niveau(Niveau::Critique).valeur("injoignable").detail(format!("{} : {e}", c.url));
        }
    };
    let statut = reponse.status().as_u16();
    let corps = if c.contient.is_some() {
        reponse.body_mut().with_config().limit(2_000_000).read_to_string().unwrap_or_default()
    } else {
        String::new()
    };
    let duree = debut.elapsed().as_secs_f64();
    let m = Mesure::new(cle, "Web", c.nom.as_str())
        .valeur(format!("{statut} en {} s", nombre(duree, 2)))
        .metrique(duree);
    if statut > c.statut_max {
        m.niveau(Niveau::Critique).detail(format!("{} répond {statut}", c.url))
    } else if let Some(t) = c.contient.as_ref().filter(|t| !corps.contains(t.as_str())) {
        m.niveau(Niveau::Critique).detail(format!("« {t} » absent de la page"))
    } else if duree > c.lent {
        m.niveau(Niveau::Attention).detail(format!("réponse lente (plus de {} s)", nombre(c.lent, 1)))
    } else {
        m
    }
}

pub fn tls(c: &Tls) -> Mesure {
    let cle = format!("tls:{}", c.hote);
    match expiration(&c.hote, c.port) {
        Ok(fin) => {
            let jours = (fin - maintenant()) as f64 / 86400.0;
            let niveau = if jours <= c.critique_jours {
                Niveau::Critique
            } else if jours <= c.attention_jours {
                Niveau::Attention
            } else {
                Niveau::Ok
            };
            let m = Mesure::new(cle, "Certificat", c.hote.as_str())
                .niveau(niveau)
                .valeur(format!("{} jours", jours.floor()))
                .metrique(jours);
            if niveau == Niveau::Ok { m } else { m.detail("le certificat expire bientôt : renouvellement raté ?") }
        }
        Err(e) => Mesure::new(cle, "Certificat", c.hote.as_str()).niveau(Niveau::Critique).valeur("invalide").detail(e),
    }
}

/// Date d'expiration (secondes Unix) du certificat présenté, après vérification complète.
fn expiration(hote: &str, port: u16) -> Result<i64, String> {
    let mut racines = rustls::RootCertStore::empty();
    racines.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(racines)
        .with_no_client_auth();
    let nom = rustls::pki_types::ServerName::try_from(hote.to_string()).map_err(|e| e.to_string())?;
    let mut connexion = rustls::ClientConnection::new(Arc::new(config), nom).map_err(|e| e.to_string())?;
    let adresse = (hote, port)
        .to_socket_addrs()
        .map_err(|e| format!("{hote} : {e}"))?
        .next()
        .ok_or_else(|| format!("{hote} : aucune adresse"))?;
    let mut flux = TcpStream::connect_timeout(&adresse, Duration::from_secs(10)).map_err(|e| format!("{hote}:{port} : {e}"))?;
    flux.set_read_timeout(Some(Duration::from_secs(10))).ok();
    flux.set_write_timeout(Some(Duration::from_secs(10))).ok();
    while connexion.is_handshaking() {
        connexion.complete_io(&mut flux).map_err(|e| e.to_string())?;
    }
    let cert = connexion.peer_certificates().and_then(|c| c.first()).ok_or("aucun certificat présenté")?;
    fin_validite(cert.as_ref()).ok_or_else(|| "certificat illisible".into())
}

/// Lit `notAfter` dans un certificat X.509 (DER) : Certificate → tbsCertificate → validity.
pub fn fin_validite(der: &[u8]) -> Option<i64> {
    let (_, cert, _) = tlv(der)?;
    let (_, tbs, _) = tlv(cert)?;
    let mut reste = tbs;
    let (tag, _, suite) = tlv(reste)?;
    if tag == 0xA0 {
        reste = suite; // version explicite
    }
    for _ in 0..3 {
        reste = tlv(reste)?.2; // numéro de série, algorithme, émetteur
    }
    let (tag, validite, _) = tlv(reste)?;
    if tag != 0x30 {
        return None;
    }
    let (_, _, apres) = tlv(validite)?; // notBefore
    let (tag, date, _) = tlv(apres)?;
    let texte = std::str::from_utf8(date).ok()?;
    match tag {
        0x17 => date_asn1(texte, false),
        0x18 => date_asn1(texte, true),
        _ => None,
    }
}

/// (étiquette, contenu, suite) d'un élément DER.
fn tlv(d: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *d.first()?;
    let premier = *d.get(1)? as usize;
    let (longueur, debut) = if premier < 0x80 {
        (premier, 2)
    } else {
        let n = premier & 0x7f;
        if n == 0 || n > 4 {
            return None;
        }
        let mut l = 0usize;
        for i in 0..n {
            l = (l << 8) | *d.get(2 + i)? as usize;
        }
        (l, 2 + n)
    };
    let fin = debut.checked_add(longueur)?;
    if fin > d.len() {
        return None;
    }
    Some((tag, &d[debut..fin], &d[fin..]))
}

/// UTCTime `AAMMJJhhmmssZ` ou GeneralizedTime `AAAAMMJJhhmmssZ` → secondes Unix.
fn date_asn1(t: &str, generalise: bool) -> Option<i64> {
    let t = t.strip_suffix('Z')?;
    let (annee, reste) = if generalise {
        (t.get(0..4)?.parse::<i64>().ok()?, t.get(4..)?)
    } else {
        let a = t.get(0..2)?.parse::<i64>().ok()?;
        (if a < 50 { 2000 + a } else { 1900 + a }, t.get(2..)?)
    };
    let n = |i: usize| reste.get(i..i + 2).and_then(|s| s.parse::<i64>().ok());
    let (mois, jour, h, m, s) = (n(0)?, n(2)?, n(4)?, n(6)?, n(8)?);
    Some(jours_depuis_epoque(annee, mois, jour) * 86400 + h * 3600 + m * 60 + s)
}

/// Algorithme de H. Hinnant (days_from_civil).
fn jours_depuis_epoque(a: i64, m: i64, j: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let ere = a.div_euclid(400);
    let ade = a - ere * 400;
    let mp = (m + 9) % 12;
    let adj = (153 * mp + 2) / 5 + j - 1;
    let ado = ade * 365 + ade / 4 - ade / 100 + adj;
    ere * 146097 + ado - 719468
}

pub fn dns(c: &Dns) -> Mesure {
    let cle = format!("dns:{}", c.nom);
    let debut = Instant::now();
    match (c.nom.as_str(), 0).to_socket_addrs() {
        Ok(adresses) => {
            let mut ips: Vec<IpAddr> = adresses.map(|a| a.ip()).collect();
            ips.sort();
            ips.dedup();
            let texte: Vec<String> = ips.iter().map(|i| i.to_string()).collect();
            let m = Mesure::new(cle, "DNS", c.nom.as_str())
                .valeur(texte.join(", "))
                .metrique(debut.elapsed().as_secs_f64());
            if ips.is_empty() {
                m.niveau(Niveau::Critique).valeur("aucune adresse")
            } else if !c.attendu.is_empty() && !c.attendu.iter().all(|a| texte.contains(a)) {
                m.niveau(Niveau::Attention).detail(format!("attendu : {}", c.attendu.join(", ")))
            } else {
                m
            }
        }
        Err(e) => Mesure::new(cle, "DNS", c.nom.as_str()).niveau(Niveau::Critique).valeur("ne résout pas").detail(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(jours_depuis_epoque(1970, 1, 1), 0);
        assert_eq!(date_asn1("270115120000Z", false), Some(1_800_014_400));
        assert_eq!(date_asn1("20270115120000Z", true), Some(1_800_014_400));
        assert_eq!(date_asn1("991231235959Z", false), Some(946_684_799));
        assert_eq!(date_asn1("n'importe", false), None);
    }

    #[test]
    fn der_minimal() {
        // Certificate { tbs { [0]{INTEGER 2}, INTEGER 1, SEQ{}, SEQ{}, SEQ{ UTCTime, UTCTime } } }
        let date = b"270115120000Z";
        let mut validite = vec![0x17, 13];
        validite.extend_from_slice(date);
        validite.extend_from_slice(&[0x17, 13]);
        validite.extend_from_slice(date);
        let mut tbs = vec![0xA0, 3, 0x02, 1, 2, 0x02, 1, 1, 0x30, 0, 0x30, 0, 0x30, validite.len() as u8];
        tbs.extend(validite);
        let mut cert = vec![0x30, tbs.len() as u8];
        cert.extend(tbs);
        let mut der = vec![0x30, 0x81, cert.len() as u8];
        der.extend(cert);
        assert_eq!(fin_validite(&der), Some(1_800_014_400));
        assert_eq!(fin_validite(&der[..10]), None);
    }
}
