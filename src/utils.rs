use std::{net::{IpAddr, SocketAddr, ToSocketAddrs}, time::{SystemTime, UNIX_EPOCH}};
use log::{debug, error, info};
use solana_gossip::contact_info::{ContactInfo, Protocol};
use solana_ledger::shred::Shred;

pub fn resolve_entrypoints() -> Result<Vec<SocketAddr>, Box<dyn std::error::Error>> {
    let entrypoint_strings = vec![
        "entrypoint.testnet.solana.com:8001",
        "entrypoint2.testnet.solana.com:8001", 
        "entrypoint3.testnet.solana.com:8001",
    ];

    let mut resolved = Vec::with_capacity(3);
    for (i, entrypoint_str) in entrypoint_strings.iter().enumerate() {
        debug!("Resolving entrypoint {}: '{}'", i+1, entrypoint_str);
        
        let addr: SocketAddr = entrypoint_str
            .to_socket_addrs()
            .map_err(|e| {
                error!("FAILED resolving '{}': {:?}", entrypoint_str, e);
                e
            })?
            .next()
            .ok_or_else(|| {
                error!("No addresses found for '{}'", entrypoint_str);
                "No addresses resolved"
            })?;
        
        resolved.push(addr);
    }
    
    Ok(resolved)
}

pub fn get_cluster_shred_version(entrypoints: &[SocketAddr], bind_address: IpAddr) -> Result<u16, Box<dyn std::error::Error>> {
    
    for entrypoint in entrypoints {
        match solana_net_utils::get_cluster_shred_version_with_binding(entrypoint, bind_address) {
            Ok(0) => continue, // Invalid
            Ok(shred_version) => {
                info!("Got shred version {} from {}", shred_version, entrypoint);
                return Ok(shred_version);
            }
            Err(e) => error!("Failed to get shred version from {}: {}", entrypoint, e),
        }
    }
    
    info!("Using default shred version: 9065");
    Ok(9065)
}

pub fn log_peer_details(
    peers: &[(ContactInfo, u64)], 
    tpu_peers: &[ContactInfo], 
    iteration: usize
) {
    // Use debug level to avoid contention with shred logs
    debug!("=== PEER DETAILS (iteration {}) ===", iteration);
    
    // Single log line for all peer summary to reduce contention
    debug!("All Peers (first 5): {}", 
        peers.iter().take(5).enumerate()
            .map(|(i, (ci, _))| format!("{}. {} (shred:{})", 
                i+1, 
                ci.pubkey().to_string().chars().take(8).collect::<String>(),
                ci.shred_version()))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    
    // Single log line for TVU peers to reduce contention  
    debug!("TVU-Enabled (first 5): {}",
        tpu_peers.iter().take(5).enumerate()
            .filter_map(|(i, ci)| {
                ci.tvu(Protocol::UDP).map(|tvu| 
                    format!("{}. {} -> {}", 
                        i+1, 
                        ci.pubkey().to_string().chars().take(8).collect::<String>(),
                        tvu))
            })
            .collect::<Vec<_>>()
            .join(" | ")
    );
    
    debug!("=== END PEER DETAILS ===");
}

pub fn parse_shred(data: &[u8]) -> Result<Shred, Box<dyn std::error::Error>> {
    let shred = Shred::new_from_serialized_shred(data.to_vec())?;
    Ok(shred)
}

pub fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}