use std::{
    net::{IpAddr, UdpSocket},
    sync::Arc,
    time::Duration,
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
            "[Plugin] Shred: Slot:{} Index:{} Type:{:?}",
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

    let bind_address: IpAddr = "64.34.80.45".parse().unwrap();

    let identity_keypair = Arc::new(Keypair::new());

    let gossip_socket = UdpSocket::bind((bind_address, 8000))?;
    let tvu_socket = UdpSocket::bind((bind_address, 8001))?;

    let gossip_node = GossipNode::new(
        identity_keypair,
        gossip_socket,
        &tvu_socket,
        bind_address,
        Network::Mainnet,
    )?;

    gossip_node.start_discovery(); // breaks when peers > 100

    println!("finished discovering");

    let mut shred_receiver = ShredReceiver::new(Arc::new(tvu_socket));

    // get the receiver BEFORE starting the sender thread to prevent race condition
    let receiver = shred_receiver.take_receiver();
    let _shred_handle = shred_receiver.start(); // Start receiving

    let mut plugin_runner = PluginRunner::new();
    plugin_runner.add_plugin(Box::new(ConsolePlugin));

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        plugin_runner.start_all().await.unwrap();

        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));

        loop {
            let receiver_clone = receiver.clone();

            let shred_result = tokio::task::spawn_blocking(move || {
                let receiver = receiver_clone.lock().unwrap();
                receiver.recv_timeout(Duration::from_secs(1))
            })
            .await;

            match shred_result {
                Ok(Ok(shred)) => {
                    plugin_runner.handle_shred(shred).await;
                }
                Ok(Err(std::sync::mpsc::RecvTimeoutError::Timeout)) => {
                    continue;
                }
                Ok(Err(std::sync::mpsc::RecvTimeoutError::Disconnected)) => {
                    println!("Shred receiver channel disconnected");
                    break;
                }
                Err(_) => {
                    println!("Error in shred receiver task");
                    break;
                }
            }
        }

        plugin_runner.stop_all().await.unwrap();
    });

    Ok(())
}
