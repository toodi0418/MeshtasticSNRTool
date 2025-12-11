use super::{TracerouteResult, Transport};
use crate::msnr_log;
use anyhow::Result;
use async_trait::async_trait;
use meshtastic::api::{ConnectedStreamApi, StreamApi, StreamHandle, state};
use meshtastic::protobufs::{Data, MeshPacket, PortNum, ToRadio, mesh_packet, to_radio};
use prost::Message;
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_serial::SerialPortBuilderExt;

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

use meshtastic::packet::PacketReceiver;

#[async_trait]
impl Transport for SerialTransport {
    async fn connect(&mut self) -> Result<PacketReceiver> {
        msnr_log!("Opening serial port {}", self.port_name);

        let port = tokio_serial::new(&self.port_name, self.baud_rate).open_native_async()?;

        let stream_handle = StreamHandle::from_stream(port);

        let stream_api = StreamApi::new();
        let (rx, connected_api) = stream_api.connect(stream_handle).await;

        let config_id = generate_rand_id();
        let configured_api = connected_api.configure(config_id).await?;

        self.api = Some(configured_api);
        Ok(rx)
    }

    async fn disconnect(&mut self) -> Result<()> {
        msnr_log!("Closing serial port {}", self.port_name);
        if let Some(api) = self.api.take() {
            api.disconnect().await?;
        }
        Ok(())
    }

    async fn send_admin(
        &mut self,
        _dest: &str,
        _admin_msg: meshtastic::protobufs::AdminMessage,
    ) -> Result<()> {
        Err(anyhow::anyhow!(
            "Admin messages not implemented for Serial transport"
        ))
    }

    async fn set_lna(&mut self, node_id: &str, enable: bool) -> Result<()> {
        if let Some(api) = &mut self.api {
            msnr_log!("Setting LNA for {} to {}", node_id, enable);

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

            let mesh_packet = MeshPacket {
                from: 0,
                to: dest,
                id: generate_rand_id(),
                rx_time: current_epoch_secs_u32(),
                want_ack: true,
                payload_variant: Some(mesh_packet::PayloadVariant::Decoded(Data {
                    portnum: port,
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

    async fn run_traceroute(&mut self, target_node_id: &str) -> Result<Vec<TracerouteResult>> {
        if let Some(api) = &mut self.api {
            msnr_log!("Sending Traceroute to {}", target_node_id);

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

            // TODO: Implement listening loop

            Ok(vec![])
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }
}
