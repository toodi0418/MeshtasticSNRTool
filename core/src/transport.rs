use anyhow::Result;
use async_trait::async_trait;

pub mod ip;
pub mod serial;

pub use ip::IpTransport;
pub use serial::SerialTransport;

#[derive(Debug, Clone)]
pub struct TracerouteResult {
    pub hop: u32,
    pub node_id: String,
    pub snr: f32,
    pub rssi: i32,
}

use meshtastic::packet::PacketReceiver;

#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<PacketReceiver>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn set_lna(&mut self, node_id: &str, enable: bool) -> Result<()>;
    async fn set_identity(&mut self, private_key: Vec<u8>) { let _ = private_key; } // Default impl does nothing
    async fn send_packet(&mut self, dest: &str, port: i32, payload: Vec<u8>) -> Result<()>;
    async fn send_admin(&mut self, dest: &str, admin_msg: meshtastic::protobufs::AdminMessage) -> Result<()>;
    async fn run_traceroute(&mut self, target_node_id: &str) -> Result<Vec<TracerouteResult>>;
}
