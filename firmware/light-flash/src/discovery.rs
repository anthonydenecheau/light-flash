//! Découverte des autres lampes sur le réseau : requête mDNS `_http._tcp` toutes les minutes,
//! seuls les services portant le TXT `light-flash=1` sont retenus (hors nous-mêmes). La liste est
//! lue par la page pour le mode groupe.

use anyhow::Result;
use esp_idf_svc::mdns::{EspMdns, Interface, Protocol, QueryResult};
use log::{info, warn};
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

/// Marqueur TXT annoncé par chaque lampe et attendu chez les autres.
pub const TXT_MARKER: (&str, &str) = ("light-flash", "1");
const MAX_RESULTS: usize = 16;
const QUERY_TIMEOUT: Duration = Duration::from_secs(3);
const REFRESH: Duration = Duration::from_secs(60);
const STACK_SIZE: usize = 6 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub name: String,
    pub hostname: String,
    pub ip: Ipv4Addr,
    pub port: u16,
}

pub type SharedPeers = Arc<Mutex<Vec<Peer>>>;

/// Le thread garde la propriété de `mdns` : l'annonce de cette lampe vit avec lui.
pub fn spawn(mdns: EspMdns, own_hostname: &'static str) -> Result<SharedPeers> {
    let peers: SharedPeers = Arc::new(Mutex::new(Vec::new()));
    let shared = peers.clone();
    thread::Builder::new()
        .name("discovery".into())
        .stack_size(STACK_SIZE)
        .spawn(move || run(mdns, own_hostname, shared))?;
    Ok(peers)
}

fn run(mdns: EspMdns, own_hostname: &str, peers: SharedPeers) {
    // Premières requêtes rapprochées, le temps que le réseau monte, puis une par minute.
    let mut waits = [10u64, 20, 30].into_iter();
    loop {
        thread::sleep(Duration::from_secs(
            waits.next().unwrap_or(REFRESH.as_secs()),
        ));
        let mut results: Vec<QueryResult> = (0..MAX_RESULTS).map(|_| empty_result()).collect();
        match mdns.query_ptr("_http", "_tcp", QUERY_TIMEOUT, MAX_RESULTS, &mut results) {
            Ok(n) => {
                let mut found: Vec<Peer> = Vec::new();
                for r in &results[..n] {
                    if let Some(p) = to_peer(r, own_hostname) {
                        if !found.iter().any(|f| f.hostname == p.hostname) {
                            found.push(p);
                        }
                    }
                }
                found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                let mut guard = peers.lock().unwrap_or_else(|p| p.into_inner());
                if *guard != found {
                    info!(
                        "lampes découvertes : {}",
                        if found.is_empty() {
                            "aucune".to_owned()
                        } else {
                            found
                                .iter()
                                .map(|p| format!("{} ({})", p.name, p.ip))
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    );
                }
                *guard = found;
            }
            Err(e) => warn!("requête mDNS : {e}"),
        }
    }
}

fn to_peer(r: &QueryResult, own_hostname: &str) -> Option<Peer> {
    if !r
        .txt
        .iter()
        .any(|(k, v)| k == TXT_MARKER.0 && v == TXT_MARKER.1)
    {
        return None;
    }
    let hostname = r
        .hostname
        .as_deref()?
        .trim_end_matches(".local")
        .trim_end_matches('.')
        .to_lowercase();
    if hostname.is_empty() || hostname == own_hostname {
        return None;
    }
    let ip = r.addr.iter().find_map(|a| match a {
        IpAddr::V4(v4) => Some(*v4),
        IpAddr::V6(_) => None,
    })?;
    let name = r
        .txt
        .iter()
        .find(|(k, _)| k == "name")
        .map(|(_, v)| v.clone())
        .or_else(|| r.instance_name.clone())
        .unwrap_or_else(|| hostname.clone());
    Some(Peer {
        name,
        hostname,
        ip,
        port: if r.port == 0 { 80 } else { r.port },
    })
}

/// `QueryResult` n'implémente pas `Default` : tampon vide pour `query_ptr`.
fn empty_result() -> QueryResult {
    QueryResult {
        instance_name: None,
        hostname: None,
        port: 0,
        txt: Vec::new(),
        addr: Vec::new(),
        interface: Interface::STA,
        ip_protocol: Protocol::V4,
    }
}
