use crate::config::{Config, TestMode, RelayTestMode, DirectTestMode};
use crate::transport::Transport;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use meshtastic::protobufs::{
    AdminMessage, admin_message, Config as MeshConfig, config, PortNum,
};
use prost::Message; // For encoding/decoding

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressState {
    pub total_progress: f32,
    pub current_round_progress: f32,
    pub status_message: String,
    pub eta_seconds: u64,
    pub snr_towards: Option<Vec<f32>>,
    pub snr_back: Option<Vec<f32>>,
    pub phase: String,
}

pub struct Engine {
    config: Config,
    transport: Box<dyn Transport>,
}

impl Engine {
    pub fn new(config: Config, transport: Box<dyn Transport>) -> Self {
        Self { config, transport }
    }

    pub async fn run<F>(&mut self, on_progress: F) -> Result<()>
    where
        F: Fn(ProgressState) + Send + Sync + 'static,
    {
        let mut rx = self.transport.connect().await?;
        
        // Inject User Identity (Client-Side Signing)
        // Private Key provided by user
        let priv_key_b64 = "EP7uGaSlaoJHVp5wYVzv5O6fQQNx+q8yb9OshyMANmU=";
        use base64::Engine;
        if let Ok(priv_bytes) = base64::prelude::BASE64_STANDARD.decode(priv_key_b64) {
             println!("Injecting User Identity (Client-Side Signing)...");
             self.transport.set_identity(priv_bytes).await;
        } else {
             println!("Error decoding private key!");
        }

        let total_cycles = self.config.cycles;

        for cycle in 0..total_cycles {
            // --- Phase 1: LNA OFF ---
            self.report_phase_start(&on_progress, cycle, total_cycles, "LNA OFF", 1);
            
            // Toggle LNA OFF
            // Toggle LNA OFF
            if let Err(e) = self.set_lna_mode(&mut rx, false).await {
                eprintln!("Error setting LNA OFF: {}", e);
                return Err(e); // Abort test
            }
            // Wait for settling
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Run Traceroute Loop
            // Run Traceroute Loop
            self.run_traceroute_phase(&mut rx, &on_progress, cycle, "LNA OFF", 1, total_cycles).await?;

            // --- Phase 2: LNA ON ---
            self.report_phase_start(&on_progress, cycle, total_cycles, "LNA ON", 2);

            // Toggle LNA ON
            // Toggle LNA ON
            if let Err(e) = self.set_lna_mode(&mut rx, true).await {
                eprintln!("Error setting LNA ON: {}", e);
                return Err(e); // Abort test
            }
            // Wait for settling
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Run Traceroute Loop
            // Run Traceroute Loop
            self.run_traceroute_phase(&mut rx, &on_progress, cycle, "LNA ON", 2, total_cycles).await?;
        }

        // Send final completion progress
        on_progress(ProgressState {
            total_progress: 1.0,
            current_round_progress: 1.0,
            status_message: "Test Completed".to_string(),
            eta_seconds: 0,
            snr_towards: None,
            snr_back: None,
            phase: "Done".to_string(),
        });

        self.transport.disconnect().await?;
        Ok(())
    }

    async fn set_lna_mode(&mut self, rx: &mut meshtastic::packet::PacketReceiver, enable: bool) -> Result<()> {
        let target_node = match self.config.topology {
            crate::config::Topology::Relay => self.config.roof_node_id.clone(),
            crate::config::Topology::Direct => {
                 self.config.target_node_id.clone()
            }
        }.unwrap_or_default();

        if !target_node.is_empty() {
             // Fetch Local Node Info first
            println!("Fetching Local Node Info...");
             let mut my_info_req = AdminMessage {
                payload_variant: Some(admin_message::PayloadVariant::GetOwnerRequest(true)),
                ..Default::default()
            };
            self.transport.send_packet(&"0".to_string(), PortNum::AdminApp as i32, my_info_req.encode_to_vec()).await?;
            
            // Wait briefly for info
            let info_timeout = Duration::from_secs(3);
            let info_start = Instant::now();
            loop {
                if info_start.elapsed() > info_timeout {
                    println!("Warning: Could not fetch local node info.");
                    break;
                }
                let sleep = tokio::time::sleep(Duration::from_millis(100));
                tokio::select! {
                    packet = rx.recv() => {
                        if let Some(p) = packet {
                           if let Some(meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_pkt)) = p.payload_variant {
                               if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(meshtastic::protobufs::Data { portnum, payload, .. })) = mesh_pkt.payload_variant {
                                    if portnum == PortNum::AdminApp as i32 {
                                        if let Ok(admin_rsp) = AdminMessage::decode(payload.as_slice()) {
                                            if let Some(admin_message::PayloadVariant::GetOwnerResponse(user)) = admin_rsp.payload_variant {
                                                println!("Local Node Identity: ID: {}, LongName: {}, ShortName: {}", user.id, user.long_name, user.short_name);
                                                println!("> Please ensure THIS ID ({}) is in the Roof Node's Admin List.", user.id);
                                                break;
                                            }
                                        }
                                    }
                               }
                           }
                        }
                    }
                    _ = sleep => {}
                }
            }

            println!("Requesting LoRa Config from {}...", target_node);

            // 1. Get Config Request
            let get_req = AdminMessage {
                payload_variant: Some(admin_message::PayloadVariant::GetConfigRequest(
                    admin_message::ConfigType::from_i32(5).unwrap() as i32
                )),
                ..Default::default()
            };
            self.transport.send_admin(&target_node, get_req.clone()).await?;

            // 2. Wait for Config Response
            let target_id = if target_node.starts_with('!') {
                 u32::from_str_radix(&target_node[1..], 16).unwrap_or(0)
            } else {
                 target_node.parse::<u32>().unwrap_or(0)
            };

            let mut current_lora_config: Option<config::LoRaConfig> = None;
            let wait_start = Instant::now();
            let timeout = Duration::from_secs(10);

            loop {
                if wait_start.elapsed() > timeout {
                    println!("WARNING: Get Config Timed Out! Aborting LNA Toggle.");
                    return Ok(()); 
                }
                
                let sleep = tokio::time::sleep(Duration::from_millis(100));
                tokio::select! {
                    result = rx.recv() => {
                        match result {
                            Some(packet) => {
                                use meshtastic::protobufs::{Data, PortNum};
                                use meshtastic::protobufs::from_radio::PayloadVariant;
                                
                                if let Some(PayloadVariant::Packet(mesh_packet)) = packet.payload_variant {
                                    if mesh_packet.from == target_id {
                                         if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(Data { portnum, payload, .. })) = mesh_packet.payload_variant {
                                             if portnum == PortNum::AdminApp as i32 {
                                                 if let Ok(admin_msg) = AdminMessage::decode(payload.as_slice()) {
                                                     if let Some(admin_message::PayloadVariant::GetConfigResponse(config)) = admin_msg.payload_variant {
                                                         if let Some(config::PayloadVariant::Lora(lora)) = config.payload_variant {
                                                             println!("Received LoRa Config. Current RX Gain: {:?}", lora.sx126x_rx_boosted_gain);
                                                             current_lora_config = Some(lora);
                                                             break;
                                                         }
                                                     }
                                                 }
                                             }
                                         }
                                    }
                                }
                            }
                            None => break,
                        }
                    }
                     _ = sleep => {}
                }
            }
            
            // 3. Modify and Set Config with Retry
            if let Some(mut lora) = current_lora_config {
                println!("Setting LNA (RX Boosted Gain) to {}...", enable);
                lora.sx126x_rx_boosted_gain = enable;
                
                let set_req = AdminMessage {
                    payload_variant: Some(admin_message::PayloadVariant::SetConfig(
                        MeshConfig {
                            payload_variant: Some(config::PayloadVariant::Lora(lora.clone())), // clone for loop use
                        }
                    )),
                    ..Default::default()
                };
                
                let mut success = false;

                for attempt in 1..=10 {
                     println!("Attempt {}/10: Setting LNA...", attempt);
                     // Use PKI-enabled send_admin
                     self.transport.send_admin(&target_node, set_req.clone()).await?;
                     println!("Set Config Request sent (PKI Encrypted). Waiting for ACK/Response...");

                     // Monitor for immediate response (Error or Success)
                     let ack_start = Instant::now();
                     let ack_timeout = Duration::from_secs(3);
                     loop {
                        if ack_start.elapsed() > ack_timeout {
                             println!("Wait for SetACK timed out (This is normal if node is silent on success).");
                             break;
                        }
                        let sleep = tokio::time::sleep(Duration::from_millis(100));
                        tokio::select! {
                            result = rx.recv() => {
                                match result {
                                    Some(packet) => {
                                        if let Some(meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet)) = packet.payload_variant {
                                             if mesh_packet.from == target_id {
                                                 // Log any packet from target during this window
                                                  if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(meshtastic::protobufs::Data { portnum, payload, .. })) = mesh_packet.payload_variant {
                                                      println!("Received packet from target on port {}: {:02X?}", portnum, payload);
                                                      // Check if it is AdminMessage
                                                      if portnum == PortNum::AdminApp as i32 {
                                                          if let Ok(admin_msg) = AdminMessage::decode(payload.as_slice()) {
                                                              println!("AdminMessage Response: {:?}", admin_msg.payload_variant);
                                                          }
                                                      }
                                                  }
                                             }
                                        }
                                    }
                                    None => break,
                                }
                            }
                            _ = sleep => {}
                        }
                     }

                     println!("Verifying..."); 

                     // 4. Read-Back Verification
                     tokio::time::sleep(Duration::from_secs(2)).await; // Wait for write to settle
                     
                     self.transport.send_packet(&target_node, PortNum::AdminApp as i32, get_req.encode_to_vec()).await?;
                     
                     let verify_start = Instant::now();
                     let mut verified = false;
                     
                     loop {
                        if verify_start.elapsed() > timeout {
                            println!("WARNING: Verification Read Timed Out! (Attempt {})", attempt);
                            break;
                        }

                        let sleep = tokio::time::sleep(Duration::from_millis(100));
                        tokio::select! {
                            result = rx.recv() => {
                                match result {
                                    Some(packet) => {
                                        use meshtastic::protobufs::{Data, PortNum};
                                        use meshtastic::protobufs::from_radio::PayloadVariant;
                                        
                                        if let Some(PayloadVariant::Packet(mesh_packet)) = packet.payload_variant {
                                            if mesh_packet.from == target_id {
                                                 if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(Data { portnum, payload, .. })) = mesh_packet.payload_variant {
                                                     if portnum == PortNum::AdminApp as i32 {
                                                         if let Ok(admin_msg) = AdminMessage::decode(payload.as_slice()) {
                                                             if let Some(admin_message::PayloadVariant::GetConfigResponse(config)) = admin_msg.payload_variant {
                                                                 if let Some(config::PayloadVariant::Lora(lora)) = config.payload_variant {
                                                                     if lora.sx126x_rx_boosted_gain == enable {
                                                                         println!("✅ LNA Setting VERIFIED! (Current: {})", lora.sx126x_rx_boosted_gain);
                                                                         verified = true;
                                                                     } else {
                                                                         println!("❌ LNA Verification FAILED! (Expected: {}, Got: {})", enable, lora.sx126x_rx_boosted_gain);
                                                                     }
                                                                     break;
                                                                 }
                                                             }
                                                         }
                                                     }
                                                 }
                                            }
                                        }
                                    }
                                    None => break,
                                }
                            }
                             _ = sleep => {}
                        }
                    }
                    
                    if verified {
                        success = true;
                        break;
                    } else {
                        println!("⚠️ Attempt {} failed. Retrying...", attempt);
                        tokio::time::sleep(Duration::from_secs(1)).await; // Backoff slightly
                    }
                }

                if !success {
                    let err_msg = format!("CRITICAL ERROR: Failed to toggle LNA to {} after 10 attempts! Aborting test.", enable);
                    println!("{}", err_msg);
                    return Err(anyhow::anyhow!(err_msg));
                }
            }
        }
        Ok(())
    }

    fn report_phase_start<F>(&self, on_progress: &F, cycle: u32, total_cycles: u32, phase_name: &str, phase_num: u8)
    where F: Fn(ProgressState) + Send + Sync + 'static {
        // Global ETA Calculation
        let total_duration_secs = (total_cycles as u64) * 2 * (self.config.phase_duration_ms as u64 / 1000);
        let passed_phases = (cycle * 2) + (phase_num as u32 - 1);
        let passed_seconds = passed_phases as u64 * (self.config.phase_duration_ms as u64 / 1000);
        let remaining = total_duration_secs.saturating_sub(passed_seconds);

        on_progress(ProgressState {
            total_progress: (passed_phases as f32) / ((total_cycles * 2) as f32),
            current_round_progress: 0.0,
            status_message: format!("Cycle {}/{}: Starting Phase {} ({})", cycle + 1, total_cycles, phase_num, phase_name),
            eta_seconds: remaining,
            snr_towards: None,
            snr_back: None,
            phase: phase_name.to_string(),
        });
    }

    async fn run_traceroute_phase<F>(
        &mut self,
        rx: &mut meshtastic::packet::PacketReceiver, 
        on_progress: &F,
        cycle: u32,
        phase_name: &str,
        phase_num: u8,
        total_cycles: u32
    ) -> Result<()>
    where
        F: Fn(ProgressState) + Send + Sync + 'static,
    {
        let start_time = Instant::now();
        let phase_duration = Duration::from_millis(self.config.phase_duration_ms as u64);
        let total_steps = self.config.phase_duration_ms / 1000; 
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        
        // Consume the first tick
        interval.tick().await;

        println!("Engine started. Config: {:?}", self.config);
        use std::io::Write;
        let _ = std::io::stdout().flush();

        loop {
            let elapsed = start_time.elapsed();
            if elapsed >= phase_duration {
                break;
            }

            let elapsed_secs = elapsed.as_secs();

            tokio::select! {
                _ = interval.tick() => {
                    let progress = elapsed_secs as f32 / total_steps as f32;
                    let remaining_in_phase = if total_steps as u64 > elapsed_secs { (total_steps as u64) - elapsed_secs } else { 0 };

                    // Global ETA
                    let total_phases = total_cycles * 2;
                    let current_global_phase_idx = (cycle * 2) + (phase_num as u32 - 1);
                    let future_phases = total_phases - current_global_phase_idx - 1;
                    let future_seconds = future_phases as u64 * (self.config.phase_duration_ms as u64 / 1000);
                    let global_remaining = remaining_in_phase + future_seconds;

                    let global_progress = (current_global_phase_idx as f32 + progress) / total_phases as f32;

                    on_progress(ProgressState {
                        total_progress: global_progress.min(0.99), 
                        current_round_progress: progress, 
                        status_message: format!("Cycle {}: {} - Step {}/{}", cycle + 1, phase_name, elapsed_secs, total_steps),
                        eta_seconds: global_remaining,
                        snr_towards: None,
                        snr_back: None,
                        phase: phase_name.to_string(),
                    });

                    // Send traceroute based on configured interval
                    let interval_secs = self.config.interval_ms / 1000;
                    
                    if interval_secs > 0 && elapsed_secs % (interval_secs as u64) == 0 {
                         // Determine target based on topology
                         let target = match self.config.topology {
                             crate::config::Topology::Relay => self.config.mountain_node_id.clone(),
                             crate::config::Topology::Direct => self.config.target_node_id.clone(),
                         }.unwrap_or_default();
                         
                         if !target.is_empty() {
                             println!("Sending traceroute to {}", target);
                             use std::io::Write;
                             let _ = std::io::stdout().flush();
                             if let Err(e) = self.transport.run_traceroute(&target).await {
                                 println!("Error sending traceroute: {}", e);
                             }
                         }
                    }
                }
                result = rx.recv() => {
                    match result {
                        Some(packet) => {
                            use meshtastic::protobufs::{MeshPacket, PortNum, Data, RouteDiscovery};
                            use meshtastic::protobufs::from_radio::PayloadVariant;
                            use prost::Message;

                            if let Some(PayloadVariant::Packet(mesh_packet)) = packet.payload_variant {
                                if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(Data { portnum, payload, .. })) = mesh_packet.payload_variant {
                                     if portnum == PortNum::TracerouteApp as i32 {
                                         match RouteDiscovery::decode(&payload[..]) {
                                             Ok(route_discovery) => {
                                                 println!("TRACEROUTE RESPONSE RECEIVED! ({})", phase_name);
                                                 
                                                 let snr_towards: Vec<f32> = route_discovery.snr_towards.iter().map(|&x| x as f32 / 4.0).collect();
                                                 let snr_back: Vec<f32> = route_discovery.snr_back.iter().map(|&x| x as f32 / 4.0).collect();

                                                 // Emit live SNR data
                                                 on_progress(ProgressState {
                                                     total_progress: (cycle as f32) / (total_cycles as f32), // Approximate
                                                     current_round_progress: 0.0, // Don't disrupt progress bar
                                                     status_message: format!("Received Result ({})", phase_name),
                                                     eta_seconds: 0, // Should use tracked value but 0 is fine for ephemeral
                                                     snr_towards: Some(snr_towards.clone()),
                                                     snr_back: Some(snr_back.clone()),
                                                     phase: phase_name.to_string(),
                                                 });
                                                 
                                                 // Parse configured Roof Node ID to check match
                                                 let roof_id_str = self.config.roof_node_id.clone().unwrap_or_default();
                                                 let roof_id = if roof_id_str.starts_with('!') {
                                                     u32::from_str_radix(&roof_id_str[1..], 16).unwrap_or(0)
                                                 } else {
                                                     roof_id_str.parse::<u32>().unwrap_or(0)
                                                 };

                                                 // Logic Validation for Relay Topology
                                                 if matches!(self.config.topology, crate::config::Topology::Relay) {
                                                     if route_discovery.route.contains(&roof_id) {
                                                         println!("✅ VALIDATION PASS: Route contains configured Roof Node ({} / {:x})", roof_id, roof_id);
                                                         
                                                         let mut record = TracerouteRecord {
                                                             timestamp: chrono::Local::now().to_rfc3339(),
                                                             cycle,
                                                             phase: phase_name.to_string(),
                                                             route: format!("{:?}", route_discovery.route),
                                                             snr_towards_1_room_roof: snr_towards.get(0).copied(),
                                                             snr_towards_2_roof_mtn: snr_towards.get(1).copied(),
                                                             snr_back_1_mtn_roof: snr_back.get(0).copied(),
                                                             snr_back_2_roof_room: snr_back.get(1).copied(),
                                                         };

                                                         if snr_towards.len() >= 2 && snr_back.len() >= 2 {
                                                             println!("--- SNR DATA (Roof <-> Mtn) ---");
                                                             println!("Roof -> Mtn : {:.2} dB", snr_towards[1]);
                                                             println!("Mtn  -> Roof: {:.2} dB", snr_back[0]);
                                                             println!("-----------------------------");
                                                         }
                                                         
                                                         if let Err(e) = self.append_csv_record(&record) {
                                                             println!("Error writing CSV: {}", e);
                                                         } else {
                                                             println!("Data saved to CSV.");
                                                         }

                                                     } else {
                                                          println!("❌ VALIDATION FAIL: Route does NOT contain configured Roof Node ({})", roof_id_str);
                                                     }
                                                 } else {
                                                     println!("SNR Towards: {:?}", snr_towards);
                                                     println!("SNR Back: {:?}", snr_back);
                                                 }

                                                 use std::io::Write;
                                                 let _ = std::io::stdout().flush();
                                             }
                                             Err(e) => println!("Failed to decode RouteDiscovery: {}", e),
                                         }
                                     }
                                }
                            }
                        }
                        None => {
                            println!("Transport channel closed unexpectedly.");
                            break; 
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct TracerouteRecord {
    timestamp: String,
    cycle: u32,
    phase: String,
    route: String,
    snr_towards_1_room_roof: Option<f32>,
    snr_towards_2_roof_mtn: Option<f32>,
    snr_back_1_mtn_roof: Option<f32>,
    snr_back_2_roof_room: Option<f32>,
}

impl Engine {
    // ... existing new and run methods ...

    fn calculate_total_duration(&self) -> Duration {
        // TODO: Calculate based on config
        Duration::from_secs(60)
    }

    fn append_csv_record(&self, record: &TracerouteRecord) -> Result<()> {
        let file_exists = std::path::Path::new(&self.config.output_path).exists();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.output_path)?;

        let mut writer = csv::WriterBuilder::new()
            .has_headers(!file_exists)
            .from_writer(file);

        writer.serialize(record)?;
        writer.flush()?;
        Ok(())
    }
}
