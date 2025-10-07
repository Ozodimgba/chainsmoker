/*
 More details: https://github.com/solana-foundation/specs/blob/main/p2p/shred.md

*  ** Common Header **
: The common header has size 0x53 (83 bytes).
! +--------+-----+-------------------+------------------+--------------------------------+
! | Offset | Size| Type              | Name             | Purpose                        |
! +--------+-----+-------------------+------------------+--------------------------------+
! | 0x00   | 64B | Ed25519 signature | signature        | Block producer signature       |
! | 0x40   |  1B | u8                | variant          | Shred variant                  |
! | 0x41   |  8B | u64               | slot             | Slot number                    |
! | 0x49   |  4B | u32               | shred_index      | Shred index                    |
! | 0x4d   |  2B | u16               | shred_version    | Shred version                  |
! | 0x4f   |  4B | u32               | fec_set_index    | FEC Set Index                  |
! +--------+-----+-------------------+------------------+--------------------------------+

*  ** Shred Variant Field **
: The shred variant identifies the shred type (data, code) and authentication mechanism (legacy, Merkle).
: The field is encoded as two 4-bit unsigned integers.
: The high 4-bit field is at bit range 4:8, and the low 4-bit field is at bit range 0:4.

! +------------+------------+--------------+-------------------+
! | High 4-bit | Low 4-bit  | Shred Type   | Authentication     |
! +------------+------------+--------------+-------------------+
! | 0x5        | 0xa        | Code         | Legacy             |
! | 0xa        | 0x5        | Data         | Legacy             |
! | 0x4        | Any        | Code         | Merkle             |
! | 0x8        | Any        | Data         | Merkle             |
! +------------+------------+--------------+-------------------+

*  ** Data Shred Header **
! +--------+-----+-------+----------------+--------------------------------+
! | Offset | Size| Type  | Name           | Purpose                        |
! +--------+-----+-------+----------------+--------------------------------+
! | 0x53   | 2B  | u16   | parent_offset  | Slot distance to parent block  |
! | 0x55   | 1B  | u8    | data_flags     | Data Flags                     |
! | 0x56   | 2B  | u16   | size           | Total Size                     |
! +--------+-----+-------+----------------+--------------------------------+


*  ** Code Shred Header **
! +--------+-----+-------+--------------------+-----------------------------------------+
! | Offset | Size| Type  | Name               | Purpose                                 |
! +--------+-----+-------+--------------------+-----------------------------------------+
! | 0x53   | 2B  | u16   | num_data_shreds    | Number of data shreds                   |
! | 0x55   | 2B  | u16   | num_coding_shreds  | Number of coding shreds                 |
! | 0x57   | 2B  | u16   | position           | Position of this shred in FEC set       |
! +--------+-----+-------+--------------------+-----------------------------------------+

*  ** Shred Packet Size **
: The maximum shred packet size is determined based on the IPv6 minimum link MTU.

! Max size for shred packet is 1228 bytes (Legacy) or 1203 bytes (Merkle).
*/

use crate::{stats::ReceiveStats, utils::parse_shred};
use log::{debug, error, info};
use solana_ledger::shred::Shred;
use std::{net::UdpSocket, sync::Arc, thread, time::Duration};
use std::sync::mpsc;

pub struct ShredReceiver {
    socket: Arc<UdpSocket>,
    sender: mpsc::Sender<Shred>,
    receiver: Option<mpsc::Receiver<Shred>>,
}

impl ShredReceiver {
    pub fn new(socket: Arc<UdpSocket>) -> Self {
        let (sender, receiver) = mpsc::channel::<Shred>();

        if let Err(e) = socket.set_nonblocking(false) {
            error!("Failed to set socket blocking: {}", e);
        }

        Self {
            socket,
            sender,
            receiver: Some(receiver), 
        }
    }

    fn process_packet(data: &[u8], sender_addr: std::net::SocketAddr, count: u64) -> Option<Shred> {
        match parse_shred(data) {
            Ok(shred) => {
                info!(
                    "SHRED #{}: Slot:{} Index:{} Type:{:?} from {}",
                    count,
                    shred.slot(),
                    shred.index(),
                    shred.shred_type(),
                    sender_addr
                );

                Some(shred)
            }
            Err(_) => {
                debug!(
                    "NON-SHRED #{}: {} bytes from {}",
                    count,
                    data.len(),
                    sender_addr
                );
                None
            }
        }
    }

    pub fn start(&mut self) -> thread::JoinHandle<()> {
        let socket = self.socket.clone();
        let sender = self.sender.clone();

        thread::spawn(move || {
            info!("Starting shred receiver...");

            let mut buffer = [0u8; 1232];
            let mut stats = ReceiveStats::new();

            loop {
                match socket.recv_from(&mut buffer) {
                    Ok((size, sender_addr)) => {
                        stats.increment();

                        if let Some(shred_data) =
                            Self::process_packet(&buffer[..size], sender_addr, stats.count)
                        {
                            // CHANGED: Use std::sync::mpsc send() instead of tokio
                            if sender.send(shred_data).is_err() {
                                error!("Output channel closed, stopping receiver");
                                break;
                            }
                        }

                        stats.maybe_log();
                    }
                    Err(e) => {
                        error!("Receive error: {}", e);
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        })
    }

    pub fn take_receiver(&mut self) -> mpsc::Receiver<Shred> {
        self.receiver.take().expect("Receiver already taken")
    }
}
