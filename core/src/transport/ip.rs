use super::{TracerouteResult, Transport};
use anyhow::Result;
use async_trait::async_trait;
use meshtastic::api::{ConnectedStreamApi, StreamApi, StreamHandle, state};
use meshtastic::protobufs::{Data, MeshPacket, PortNum, ToRadio, mesh_packet, to_radio};
use prost::Message;
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;

pub struct IpTransport {
    ip: String,
    port: u16,
    api: Option<ConnectedStreamApi<state::Configured>>,
}

impl IpTransport {
    pub fn new(ip: String, port: u16) -> Self {
        Self {
            ip,
            port,
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

use meshtastic::packet::PacketReceiver;

#[async_trait]
impl Transport for IpTransport {
    async fn connect(&mut self) -> Result<PacketReceiver> {
        let addr = format!("{}:{}", self.ip, self.port);
        println!("Connecting to {}... (Timeout 10s)", addr);
        // Add timeout to connect
        let stream =
            tokio::time::timeout(std::time::Duration::from_secs(10), TcpStream::connect(addr))
                .await??;
        println!("TCP Connected!");

        let stream_handle = StreamHandle::from_stream(stream);

        let stream_api = StreamApi::new();
        let (rx, connected_api) = stream_api.connect(stream_handle).await;

        let config_id = generate_rand_id();
        let configured_api = connected_api.configure(config_id).await?;

        self.api = Some(configured_api);

        Ok(rx)
    }

    async fn disconnect(&mut self) -> Result<()> {
        println!("Disconnecting from {}:{}", self.ip, self.port);
        if let Some(api) = self.api.take() {
            api.disconnect().await?;
        }
        Ok(())
    }

    async fn set_lna(&mut self, node_id: &str, enable: bool) -> Result<()> {
        if let Some(api) = &mut self.api {
            println!("Setting LNA for {} to {}", node_id, enable);

            // Construct HardwareMessage to toggle GPIO
            // Assuming LNA is on GPIO 1 (needs configuration)
            let gpio_mask = 1 << 1;
            let gpio_value = if enable { gpio_mask } else { 0 };

            let hardware_msg = meshtastic::protobufs::HardwareMessage {
                r#type: meshtastic::protobufs::hardware_message::Type::WriteGpios as i32,
                gpio_mask,
                gpio_value,
            };

            let payload = hardware_msg.encode_to_vec();

            // Parse destination node_id (assuming decimal string or hex)
            // For now, if node_id is "broadcast" or empty, use broadcast, else parse
            let dest = if node_id.is_empty() || node_id == "broadcast" {
                u32::MAX
            } else if node_id.starts_with('!') {
                u32::from_str_radix(&node_id[1..], 16).unwrap_or(u32::MAX)
            } else {
                node_id.parse::<u32>().unwrap_or(u32::MAX)
            };

            let mesh_packet = MeshPacket {
                from: 0,
                to: dest,
                id: generate_rand_id(),
                rx_time: current_epoch_secs_u32(),
                want_ack: true,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(Data {
                    portnum: PortNum::RemoteHardwareApp as i32,
                    payload,
                    want_response: true,
                    ..Default::default()
                })),
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

    async fn send_packet(&mut self, dest_str: &str, port: i32, payload: Vec<u8>) -> Result<()> {
        if let Some(api) = &mut self.api {
            let dest = if dest_str.is_empty() || dest_str == "broadcast" {
                u32::MAX
            } else if dest_str.starts_with('!') {
                u32::from_str_radix(&dest_str[1..], 16).unwrap_or(u32::MAX)
            } else {
                dest_str.parse::<u32>().unwrap_or(u32::MAX)
            };

            let data = Data {
                portnum: port,
                payload,
                want_response: true,
                ..Default::default()
            };

            let mesh_packet = MeshPacket {
                from: 0,
                to: dest,
                id: generate_rand_id(),
                rx_time: current_epoch_secs_u32(),
                want_ack: true,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(data)),
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

    async fn send_admin(
        &mut self,
        dest_str: &str,
        admin_msg: meshtastic::protobufs::AdminMessage,
    ) -> Result<()> {
        if let Some(api) = &mut self.api {
            let dest = if dest_str.starts_with('!') {
                u32::from_str_radix(&dest_str[1..], 16).unwrap_or(u32::MAX)
            } else {
                dest_str.parse::<u32>().unwrap_or(u32::MAX)
            };

            // Encode AdminMessage
            let data_payload = admin_msg.encode_to_vec();

            let data = Data {
                portnum: PortNum::AdminApp as i32,
                payload: data_payload,
                want_response: true,
                dest,
                source: 0,
                ..Default::default()
            };

            let mesh_packet = MeshPacket {
                from: 0,
                to: dest,
                id: generate_rand_id(),
                rx_time: current_epoch_secs_u32(),
                want_ack: true,
                priority: mesh_packet::Priority::Reliable as i32,
                // KEY FIX: Enable Hardware-based PKI Encryption/Signing
                pki_encrypted: true,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(data)),
                ..Default::default()
            };

            let to_radio = ToRadio {
                payload_variant: Some(to_radio::PayloadVariant::Packet(mesh_packet)),
            };

            println!("Sending Admin PKI Packet to {}", dest_str);
            api.send_to_radio_packet(to_radio.payload_variant).await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    async fn run_traceroute(&mut self, target_node_id: &str) -> Result<Vec<TracerouteResult>> {
        if let Some(api) = &mut self.api {
            println!("Sending Traceroute to {}", target_node_id);

            let dest = if target_node_id.starts_with('!') {
                u32::from_str_radix(&target_node_id[1..], 16).unwrap_or(u32::MAX)
            } else {
                target_node_id.parse::<u32>().unwrap_or(u32::MAX)
            };

            let route_discovery = meshtastic::protobufs::RouteDiscovery {
                route: vec![],
                route_back: vec![],
                snr_back: vec![],
                snr_towards: vec![],
            };

            let payload = route_discovery.encode_to_vec();

            let mesh_packet = MeshPacket {
                from: 0,
                to: dest,
                id: generate_rand_id(),
                rx_time: current_epoch_secs_u32(),
                want_ack: true,
                hop_limit: 6,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(Data {
                    portnum: PortNum::TracerouteApp as i32,
                    payload,
                    want_response: true,
                    ..Default::default()
                })),
                ..Default::default()
            };

            let to_radio = ToRadio {
                payload_variant: Some(to_radio::PayloadVariant::Packet(mesh_packet)),
            };

            api.send_to_radio_packet(to_radio.payload_variant).await?;

            // TODO: Implement listening loop to capture RouteDiscovery response
            // For now, we just send the request. Capturing response requires
            // access to the read channel which is consumed by the main loop or needs a subscription mechanism.
            // Since `api` is `ConnectedStreamApi`, we don't have direct access to `rx` here as it was returned in `connect`.
            // We might need to restructure `IpTransport` to hold the `rx` or use a shared channel.

            Ok(vec![])
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }
}
