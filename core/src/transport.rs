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

#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn set_lna(&mut self, node_id: &str, enable: bool) -> Result<()>;
    async fn run_traceroute(&mut self, target_node_id: &str) -> Result<Vec<TracerouteResult>>;
}
