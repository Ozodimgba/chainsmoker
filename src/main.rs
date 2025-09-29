use std::{
    net::{IpAddr, UdpSocket},
    sync::Arc,
};

use chainsmoker::{
    Keypair, Shred,
    gossip::GossipNode,
    output::{OutputPlugin, PluginRunner},
    shred::ShredReceiver,
    types::Network,
};

// simple console plugin can be grpc/quinn but just console as example
struct ConsolePlugin;

#[async_trait::async_trait]
impl OutputPlugin for ConsolePlugin {
    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Console plugin started");
        Ok(())
    }

    async fn handle_shred(&mut self, shred: Shred) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "Shred: Slot:{} Index:{} Type:{:?}",
            shred.slot(),
            shred.index(),
            shred.shred_type()
        );
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Console plugin stopped");
        Ok(())
    }

    fn name(&self) -> &str {
        "Console"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    solana_logger::setup_with_default("chainsmoker=info,solana_gossip=warn,solana_metrics=error");

    // Your own public IP
    let bind_address: IpAddr = "192.168.61.58".parse().unwrap();

    let identity_keypair = Arc::new(Keypair::new());

    let tvu_socket = UdpSocket::bind((bind_address, 8000))?;
    let gossip_socket = UdpSocket::bind((bind_address, 8001))?;

    let gossip_node = GossipNode::new(
        identity_keypair,
        gossip_socket,
        &tvu_socket,
        bind_address,
        Network::Testnet,
    )?;

    gossip_node.start_discovery();

    let mut shred_receiver = ShredReceiver::new(Arc::new(tvu_socket));
    let mut receiver = shred_receiver.take_receiver(); // Get the channel
    let _shred_handle = shred_receiver.start(); // Start receiving

    let mut plugin_runner = PluginRunner::new();
    plugin_runner.add_plugin(Box::new(ConsolePlugin));

    // Spawn output loop in existing tokio runtime
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            plugin_runner.start_all().await.unwrap();

            while let Some(shred) = receiver.recv().await {
                plugin_runner.handle_shred(shred).await;
            }

            plugin_runner.stop_all().await.unwrap();
        });
    });

    std::thread::park();

    Ok(())
}
