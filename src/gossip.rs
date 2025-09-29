use std::{
    net::{IpAddr, UdpSocket},
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::Duration,
};

use solana_gossip::{
    cluster_info::ClusterInfo,
    contact_info::{ContactInfo, Protocol},
    gossip_service::GossipService,
};
use solana_sdk::signer::{Signer, keypair::Keypair};

use log::{debug, info};
use solana_streamer::socket::SocketAddrSpace;

use crate::{types::Network, utils::*};

pub struct GossipNode {
    pub cluster_info: Arc<ClusterInfo>,
    pub gossip_service: GossipService,
    pub exit: Arc<AtomicBool>,
}

impl GossipNode {
    pub fn new(
        identity_keypair: Arc<Keypair>,
        gossip_socket: UdpSocket,
        tvu_socket: &UdpSocket,
        bind_address: IpAddr,
        network: Network,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let pubkey = identity_keypair.pubkey();
        let gossip_addr = gossip_socket.local_addr()?;
        let tvu_addr = tvu_socket.local_addr()?;

        debug!("Node identity: {}", pubkey);
        debug!("Gossip address: {}", gossip_addr);
        debug!("TVU address: {}", tvu_addr);

        let entrypoints = resolve_entrypoints(network)?;
        let shred_version = get_cluster_shred_version(&entrypoints, bind_address)?;

        let mut contact_info = ClusterInfo::gossip_contact_info(pubkey, gossip_addr, shred_version);

        // Set TVU address
        contact_info.set_tvu(Protocol::UDP, tvu_addr)?;

        let mut cluster_info =
            ClusterInfo::new(contact_info, identity_keypair, SocketAddrSpace::Unspecified);

        let mut entrypoint_contacts = Vec::new();
        for addr in entrypoints {
            let contact = ContactInfo::new_gossip_entry_point(&addr);
            entrypoint_contacts.push(contact);
        }
        cluster_info.set_entrypoints(entrypoint_contacts);

        let temp_dir = std::env::temp_dir().join(format!("solana-gossip-{}", pubkey));
        std::fs::create_dir_all(&temp_dir)?;
        cluster_info.restore_contact_info(&temp_dir, 0);

        let cluster_info = Arc::new(cluster_info);
        let exit = Arc::new(AtomicBool::new(false));

        let gossip_service = GossipService::new(
            &cluster_info,
            None, // No bank forks
            Arc::from(vec![gossip_socket]),
            None, // No gossip validators filter
            true, // Check duplicate instance
            None, // No CRDS filters
            exit.clone(),
        );

        Ok(Self {
            cluster_info,
            gossip_service,
            exit,
        })
    }

    pub fn start_discovery(&self) {
        info!("Starting gossip discovery...");

        let cluster_info = self.cluster_info.clone();

        let mut iteration = 0;
        loop {
            thread::sleep(Duration::from_secs(1));

            let peers = cluster_info.all_peers();
            let tpu_peers = cluster_info.tpu_peers();

            iteration += 1;
            info!(
                "Discovery [{:02}s]: {} total peers, {} with TVU",
                iteration,
                peers.len(),
                tpu_peers.len()
            );

            // Detailed logging every 10 seconds
            if iteration > 0 && iteration % 10 == 0 {
                log_peer_details(&peers, &tpu_peers, iteration);
            }

            // if peers.len() > 100 {
            //     info!("   Successfully joined Solana gossip network!");
            //     info!("   Validators discovered: {}", peers.len());
            //     info!("   With TVU endpoints: {}", tpu_peers.len());
            //     break;
            // }
        }
    }
}
