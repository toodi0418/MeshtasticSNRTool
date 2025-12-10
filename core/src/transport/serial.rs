use super::{Transport, TracerouteResult};
use anyhow::Result;
use async_trait::async_trait;
use meshtastic::api::{state, ConnectedStreamApi, StreamApi, StreamHandle};
use meshtastic::protobufs::{self, mesh_packet, to_radio, Data, MeshPacket, PortNum, ToRadio};
use prost::Message;
use tokio_serial::SerialPortBuilderExt;
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SerialTransport {
    port_name: String,
    baud_rate: u32,
    api: Option<ConnectedStreamApi<state::Configured>>,
}

impl SerialTransport {
    pub fn new(port_name: String) -> Self {
        Self {
            port_name,
            baud_rate: 115200,
            api: None,
        }
    }
}

fn generate_rand_id() -> u32 {
    let mut rng = rand::rng();
    rng.random()
}

fn current_epoch_secs_u32() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32
}

#[async_trait]
impl Transport for SerialTransport {
    async fn connect(&mut self) -> Result<()> {
        println!("Opening serial port {}", self.port_name);
        
        let port = tokio_serial::new(&self.port_name, self.baud_rate)
            .open_native_async()?;
            
        let stream_handle = StreamHandle::from_stream(port);
        
        let stream_api = StreamApi::new();
        let (_rx, connected_api) = stream_api.connect(stream_handle).await;
        
        let config_id = generate_rand_id();
        let configured_api = connected_api.configure(config_id).await?;
        
        self.api = Some(configured_api);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        println!("Closing serial port {}", self.port_name);
        if let Some(api) = self.api.take() {
            api.disconnect().await?;
        }
        Ok(())
    }

    async fn set_lna(&mut self, node_id: &str, enable: bool) -> Result<()> {
        if let Some(api) = &mut self.api {
             println!("Setting LNA for {} to {} via Serial", node_id, enable);
             
             let payload = b"LNA_TOGGLE".to_vec();
             let mesh_packet = MeshPacket {
                from: 0,
                to: u32::MAX,
                id: generate_rand_id(),
                rx_time: current_epoch_secs_u32(),
                want_ack: false,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(
                    Data {
                        portnum: PortNum::TextMessageApp as i32,
                        payload,
                        ..Default::default()
                    }
                )),
                ..Default::default()
             };
             let to_radio = ToRadio {
                 payload_variant: Some(to_radio::PayloadVariant::Packet(mesh_packet)),
             };
             api.send_to_radio_packet(to_radio.payload_variant).await?;
             Ok(())
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    async fn run_traceroute(&mut self, target_node_id: &str) -> Result<Vec<TracerouteResult>> {
        if let Some(_api) = &mut self.api {
            println!("Running traceroute to {} via Serial", target_node_id);
            Ok(vec![])
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }
}
