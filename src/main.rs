use solana_gossip::{
    cluster_info::ClusterInfo, 
    contact_info::{ContactInfo, Protocol}, 
    gossip_service::GossipService,
};
use solana_sdk::{
    signer::{Signer, keypair::Keypair}, 
};
use solana_streamer::socket::SocketAddrSpace;
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering}, Arc
    },
    thread,
    time::Duration,
};
use solana_ledger::shred::{Shred};


fn get_local_ip() -> Result<IpAddr, Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?;
    Ok(socket.local_addr()?.ip())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let identity_keypair = Arc::new(Keypair::new());
    let pubkey = identity_keypair.pubkey();
    let local_ip = get_local_ip()?;
    // let local_ip: IpAddr = "172.225.99.254".parse()?;
    
    println!("Local IP: {:?}", local_ip);
    println!("Node identity: {}", pubkey);

    let gossip_socket = UdpSocket::bind((local_ip, 8000))?;
    let gossip_addr = gossip_socket.local_addr()?;
    println!("Gossip address: {}", gossip_addr);

    let tvu_socket = UdpSocket::bind((local_ip, 8001))?;
    let tvu_addr = tvu_socket.local_addr()?;
    println!("TVU listening on: {}", tvu_addr);

    let entrypoint_strings = vec![
        "entrypoint.testnet.solana.com:8001",
        "entrypoint2.testnet.solana.com:8001",
        "entrypoint3.testnet.solana.com:8001",
    ];

    let mut resolved_entrypoints = Vec::new();
    for (i, entrypoint_str) in entrypoint_strings.iter().enumerate() {
        println!("Resolving entrypoint {}: '{}'", i+1, entrypoint_str);
        
        let entrypoint_addr: SocketAddr = entrypoint_str
            .to_socket_addrs()
            .map_err(|e| {
                println!("FAILED resolving '{}': {:?}", entrypoint_str, e);
                e
            })?
            .next()
            .ok_or_else(|| {
                println!("No addresses found for '{}'", entrypoint_str);
                "No addresses resolved"
            })?;
        
        resolved_entrypoints.push(entrypoint_addr);
    }

    let expected_shred_version = get_cluster_shred_version(&resolved_entrypoints, local_ip)
        .unwrap_or(9065);
    
    println!("Using shred version: {}", expected_shred_version);

    // FIX #1: Create contact_info with TVU from the start
    let mut contact_info = ClusterInfo::gossip_contact_info(
        pubkey,
        gossip_addr, 
        expected_shred_version,
    );
    
    // FIX #2: Set TVU on the contact info before creating ClusterInfo
    contact_info.set_tvu(Protocol::UDP, tvu_addr)?;

    let mut cluster_info = ClusterInfo::new(
        contact_info, 
        identity_keypair,
        SocketAddrSpace::Unspecified
    );

    let mut entrypoints = Vec::new();
    for entrypoint_addr in resolved_entrypoints {
        let entrypoint_contact = ContactInfo::new_gossip_entry_point(&entrypoint_addr);
        entrypoints.push(entrypoint_contact);
        println!("Added entrypoint: {}", entrypoint_addr);
    }

    cluster_info.set_entrypoints(entrypoints);
    
    let temp_dir = std::env::temp_dir().join(format!("solana-gossip-{}", pubkey));
    std::fs::create_dir_all(&temp_dir)?;
    cluster_info.restore_contact_info(&temp_dir, 0);

    let cluster_info = Arc::new(cluster_info);

    // FIX #3: Start shred receiver (you defined it but never called it)
    let tvu_socket_arc = Arc::new(tvu_socket);
    let _shred_receiver_handle = start_shred_receiver(tvu_socket_arc.clone());

    let exit = Arc::new(AtomicBool::new(false));
    let _gossip_service = GossipService::new(
        &cluster_info,
        None,// No bank forks
        Arc::from(vec![gossip_socket]),// Arc<[UdpSocket]> format
        None,// No gossip validators filter
        true,// Check duplicate instance
        None,// No CRDS filters
        exit.clone(),
    );

    println!("Starting gossip with shred version {}...", expected_shred_version);

    for iteration in 0..120 {
        thread::sleep(Duration::from_secs(1));
        
        let peers = cluster_info.all_peers();
        let tpu_peers = cluster_info.tpu_peers();

        
        println!("Discovery [{:02}s]: {} total peers, {} with TVU", 
                iteration + 1, peers.len(), tpu_peers.len());

        if iteration > 0 && iteration % 10 == 0 {
        println!("=== PEER DETAILS (iteration {}) ===", iteration);
        
        // Log first 5 peers with full details
        println!("All Peers (showing first 5):");
        for (i, (contact_info, _timestamp)) in peers.iter().enumerate() {
            println!("  {}. Validator: {}", i+1, contact_info.pubkey());
            
            if let Some(gossip_addr) = contact_info.gossip() {
                println!("     Gossip: {}", gossip_addr);
            }
            
            if let Some(tvu_addr) = contact_info.tvu(Protocol::UDP) {
                println!("     TVU: {}", tvu_addr);
            }
            
            if let Some(tpu_addr) = contact_info.tpu(Protocol::UDP) {
                println!("     TPU: {}", tpu_addr);
            }
            
            println!("     Shred Version: {}", contact_info.shred_version());
            println!("     Last Updated: {}", contact_info.wallclock());
            println!();
        }
        
        // Log TVU-enabled peers (these are your shred sources!)
        println!("TVU-Enabled Validators (showing first 5):");
        for (i, contact_info) in tpu_peers.iter().take(5).enumerate() {
            if let Some(tvu_addr) = contact_info.tvu(Protocol::UDP) {
                println!("  {}. {} -> TVU: {}", 
                        i+1, contact_info.pubkey(), tvu_addr);
            }
        }
        println!("=== END PEER DETAILS ===\n");
    }
        
        if peers.len() > 100 {
            println!("Successfully joined Solana gossip network!");
            println!("Validators discovered: {}", peers.len());
            println!("With TVU endpoints: {}", tpu_peers.len());
            break;
        }
    }

    cluster_info.save_contact_info();
    exit.store(true, Ordering::Relaxed);
    std::fs::remove_dir_all(&temp_dir).ok();

    Ok(())
}

fn get_cluster_shred_version(entrypoints: &[SocketAddr], bind_address: IpAddr) -> Option<u16> {
    for entrypoint in entrypoints {
        match solana_net_utils::get_cluster_shred_version_with_binding(entrypoint, bind_address) {
            Ok(0) => continue, // Invalid
            Ok(shred_version) => {
                println!("Obtained shred version {} from {}", shred_version, entrypoint);
                return Some(shred_version);
            }
            Err(e) => println!("Failed to get shred version from {}: {}", entrypoint, e),
        }
    }
    None
}

fn start_shred_receiver(socket: Arc<UdpSocket>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0u8; 1232]; // Max shred size
        let mut shred_count = 0u64;
        
        println!("Starting shred receiver...");
        
        loop {
            match socket.recv_from(&mut buffer) {
                Ok((size, sender_addr)) => {
                    shred_count += 1;
                    
                    // Try to parse as shred
                    match parse_shred(&buffer[..size]) {
                        Ok(shred_info) => {
                            println!("SHRED #{}: {} from {} | Slot: {} | Index: {} | Type: {:?}", 
                                shred_count,
                                size,
                                sender_addr,
                                shred_info.slot(),
                                shred_info.index(),
                                shred_info.shred_type()
                            );
                        }
                        Err(_) => {
                            println!("NON-SHRED #{}: {} bytes from {}", 
                                shred_count, size, sender_addr);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Receive error: {}", e);
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    })
}

pub fn parse_shred(data: &[u8]) -> Result<Shred, Box<dyn std::error::Error>> {
    let shred = Shred::new_from_serialized_shred(data.to_vec())?;
    
    Ok(shred)
}