use crate::config::{Config, LnaControlTarget};
use crate::transport::Transport;
use crate::{msnr_log, msnr_log_err};
use anyhow::Result;
use meshtastic::protobufs::{AdminMessage, Config as MeshConfig, PortNum, admin_message, config};
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant}; // For encoding/decoding

const LNA_MAX_ATTEMPTS: u32 = 10;
const LNA_WAIT_TIMEOUT_SECS: u64 = 30;
const LNA_ACK_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressState {
    pub total_progress: f32,
    pub current_round_progress: f32,
    pub status_message: String,
    pub eta_seconds: u64,
    pub snr_towards: Option<Vec<f32>>,
    pub snr_back: Option<Vec<f32>>,
    pub phase: String,
    pub average_stats: Option<AverageStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AverageStats {
    pub lna_off_samples: u32,
    pub lna_off_roof_to_mtn: Option<f32>,
    pub lna_off_mtn_to_roof: Option<f32>,
    pub lna_on_samples: u32,
    pub lna_on_roof_to_mtn: Option<f32>,
    pub lna_on_mtn_to_roof: Option<f32>,
}

impl AverageStats {
    pub fn delta_roof_to_mtn(&self) -> Option<f32> {
        match (self.lna_on_roof_to_mtn, self.lna_off_roof_to_mtn) {
            (Some(on), Some(off)) => Some(on - off),
            _ => None,
        }
    }

    pub fn delta_mtn_to_roof(&self) -> Option<f32> {
        match (self.lna_on_mtn_to_roof, self.lna_off_mtn_to_roof) {
            (Some(on), Some(off)) => Some(on - off),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
struct ChannelStats {
    samples: u32,
    sum: f32,
}

impl ChannelStats {
    fn add_sample(&mut self, value: f32) {
        self.sum += value;
        self.samples += 1;
    }

    fn average(&self) -> Option<f32> {
        if self.samples == 0 {
            None
        } else {
            Some(self.sum / self.samples as f32)
        }
    }
}

#[derive(Debug, Default)]
struct PhaseStats {
    roof_to_mtn: ChannelStats,
    mtn_to_roof: ChannelStats,
}

impl PhaseStats {
    fn add_sample(&mut self, roof_to_mtn: Option<f32>, mtn_to_roof: Option<f32>) {
        if let Some(val) = roof_to_mtn {
            self.roof_to_mtn.add_sample(val);
        }
        if let Some(val) = mtn_to_roof {
            self.mtn_to_roof.add_sample(val);
        }
    }

    fn average_roof_to_mtn(&self) -> Option<f32> {
        self.roof_to_mtn.average()
    }

    fn average_mtn_to_roof(&self) -> Option<f32> {
        self.mtn_to_roof.average()
    }

    fn count_roof_to_mtn(&self) -> u32 {
        self.roof_to_mtn.samples
    }
}

#[derive(Debug)]
enum RouteValidationOutcome {
    RoofOnly,
}

pub struct Engine {
    config: Config,
    transport: Box<dyn Transport>,
    session_keys: HashMap<String, Vec<u8>>,
    stats_lna_on: PhaseStats,
    stats_lna_off: PhaseStats,
}

impl Engine {
    pub fn new(config: Config, transport: Box<dyn Transport>) -> Self {
        Self {
            config,
            transport,
            session_keys: HashMap::new(),
            stats_lna_on: PhaseStats::default(),
            stats_lna_off: PhaseStats::default(),
        }
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
            msnr_log!("Injecting User Identity (Client-Side Signing)...");
            self.transport.set_identity(priv_bytes).await;
        } else {
            msnr_log!("Error decoding private key!");
        }

        let total_cycles = self.config.cycles;

        for cycle in 0..total_cycles {
            // --- Phase 1: LNA OFF ---
            self.report_phase_start(&on_progress, cycle, total_cycles, "LNA OFF", 1);

            // Toggle LNA OFF
            // Toggle LNA OFF
            if let Err(e) = self.set_lna_mode(&mut rx, false).await {
                msnr_log_err!("Error setting LNA OFF: {}", e);
                return Err(e); // Abort test
            }
            // Wait for settling
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Run Traceroute Loop
            // Run Traceroute Loop
            self.run_traceroute_phase(
                &mut rx,
                &on_progress,
                cycle,
                "LNA OFF",
                1,
                total_cycles,
                false,
            )
            .await?;

            // --- Phase 2: LNA ON ---
            self.report_phase_start(&on_progress, cycle, total_cycles, "LNA ON", 2);

            // Toggle LNA ON
            // Toggle LNA ON
            if let Err(e) = self.set_lna_mode(&mut rx, true).await {
                msnr_log_err!("Error setting LNA ON: {}", e);
                return Err(e); // Abort test
            }
            // Wait for settling
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Run Traceroute Loop
            // Run Traceroute Loop
            self.run_traceroute_phase(
                &mut rx,
                &on_progress,
                cycle,
                "LNA ON",
                2,
                total_cycles,
                true,
            )
            .await?;
        }

        self.log_average_summary();

        // Send final completion progress
        on_progress(ProgressState {
            total_progress: 1.0,
            current_round_progress: 1.0,
            status_message: "Test Completed".to_string(),
            eta_seconds: 0,
            snr_towards: None,
            snr_back: None,
            phase: "Done".to_string(),
            average_stats: Some(self.current_average_stats()),
        });

        if let Err(e) = self.transport.disconnect().await {
            msnr_log!("Warning: Failed to disconnect cleanly: {e}");
        }
        Ok(())
    }

    async fn set_lna_mode(
        &mut self,
        rx: &mut meshtastic::packet::PacketReceiver,
        enable: bool,
    ) -> Result<()> {
        let target_node = match self.resolve_lna_control_node() {
            Some(node) if !node.is_empty() => node,
            _ => {
                msnr_log!("LNA control disabled or missing target node, skipping toggle.");
                return Ok(());
            }
        };

        let target_id = if target_node.starts_with('!') {
            u32::from_str_radix(&target_node[1..], 16).unwrap_or(0)
        } else {
            target_node.parse::<u32>().unwrap_or(0)
        };

        msnr_log!("Fetching Local Node Info...");
        let owner_req = AdminMessage {
            payload_variant: Some(admin_message::PayloadVariant::GetOwnerRequest(true)),
            ..Default::default()
        };
        self.send_admin_with_session("0", &owner_req).await?;

        let info_timeout = Duration::from_secs(3);
        let info_start = Instant::now();
        loop {
            if info_start.elapsed() > info_timeout {
                msnr_log!("Warning: Could not fetch local node info.");
                break;
            }
            let sleep = tokio::time::sleep(Duration::from_millis(100));
            tokio::select! {
                packet = rx.recv() => {
                    if let Some(p) = packet {
                        self.remember_session_key_from_packet(&p);
                        if let Some(meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_pkt)) = p.payload_variant {
                            if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(meshtastic::protobufs::Data { portnum, payload, .. })) = mesh_pkt.payload_variant {
                                if portnum == PortNum::AdminApp as i32 {
                                    if let Ok(admin_rsp) = AdminMessage::decode(payload.as_slice()) {
                                            if let Some(admin_message::PayloadVariant::GetOwnerResponse(user)) = admin_rsp.payload_variant {
                                                msnr_log!("Local Node Identity: ID: {}, LongName: {}, ShortName: {}", user.id, user.long_name, user.short_name);
                                                msnr_log!("> Please ensure THIS ID ({}) is in the Roof Node's Admin List.", user.id);

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

        if !self.has_session_key(&target_node) {
            let session_req = AdminMessage {
                payload_variant: Some(admin_message::PayloadVariant::GetConfigRequest(
                    admin_message::ConfigType::SessionkeyConfig as i32,
                )),
                ..Default::default()
            };
            self.send_admin_with_session(&target_node, &session_req)
                .await?;
        }

        let get_req = AdminMessage {
            payload_variant: Some(admin_message::PayloadVariant::GetConfigRequest(
                admin_message::ConfigType::LoraConfig as i32,
            )),
            ..Default::default()
        };

        let mut lora = self
            .fetch_lora_config_with_retry(rx, &target_node, target_id, &get_req, LNA_MAX_ATTEMPTS)
            .await?;

        msnr_log!(
            "Received LoRa Config. Current RX Gain: {:?}",
            lora.sx126x_rx_boosted_gain
        );

        msnr_log!("Setting LNA (RX Boosted Gain) to {}...", enable);
        lora.sx126x_rx_boosted_gain = enable;

        let set_req = AdminMessage {
            payload_variant: Some(admin_message::PayloadVariant::SetConfig(MeshConfig {
                payload_variant: Some(config::PayloadVariant::Lora(lora.clone())),
            })),
            ..Default::default()
        };

        let mut success = false;

        for attempt in 1..=10 {
            msnr_log!("Attempt {}/10: Setting LNA...", attempt);
            self.send_admin_with_session(&target_node, &set_req).await?;
            msnr_log!("Set Config Request sent (PKI Encrypted). Waiting for ACK/Response...");

            let ack_start = Instant::now();
            let ack_timeout = Duration::from_secs(LNA_ACK_TIMEOUT_SECS);
            loop {
                if ack_start.elapsed() > ack_timeout {
                    msnr_log!(
                        "Wait for SetACK timed out (This is normal if node is silent on success)."
                    );
                    break;
                }
                let sleep = tokio::time::sleep(Duration::from_millis(100));
                tokio::select! {
                    result = rx.recv() => {
                        match result {
                            Some(packet) => {
                                self.remember_session_key_from_packet(&packet);
                                if let Some(meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet)) = packet.payload_variant {
                                    if mesh_packet.from == target_id {
                                        if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(meshtastic::protobufs::Data { portnum, payload, .. })) = mesh_packet.payload_variant {
                                            msnr_log!("Received packet from target on port {}: {:02X?}", portnum, payload);
                                            if portnum == PortNum::AdminApp as i32 {
                                                if let Ok(admin_msg) = AdminMessage::decode(payload.as_slice()) {
                                                    msnr_log!("AdminMessage Response: {:?}", admin_msg.payload_variant);
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

            msnr_log!("Verifying...");
            tokio::time::sleep(Duration::from_secs(2)).await;

            let verify_result = self
                .fetch_lora_config_once(rx, &target_node, target_id, &get_req)
                .await;

            let mut verified = false;
            match verify_result {
                Ok(config_lora) => {
                    if config_lora.sx126x_rx_boosted_gain == enable {
                        msnr_log!(
                            "✅ LNA Setting VERIFIED! (Current: {})",
                            config_lora.sx126x_rx_boosted_gain
                        );
                        verified = true;
                    } else {
                        msnr_log!(
                            "❌ LNA Verification FAILED! (Expected: {}, Got: {})",
                            enable,
                            config_lora.sx126x_rx_boosted_gain
                        );
                    }
                }
                Err(e) => {
                    msnr_log!(
                        "WARNING: Verification Read Timed Out! (Attempt {}) | {}",
                        attempt,
                        e
                    );
                }
            }

            if verified {
                success = true;
                break;
            } else {
                msnr_log!("⚠️ Attempt {} failed. Retrying...", attempt);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        if !success {
            let err_msg = format!(
                "CRITICAL ERROR: Failed to toggle LNA to {} after 10 attempts! Aborting test.",
                enable
            );
            msnr_log!("{}", err_msg);
            return Err(anyhow::anyhow!(err_msg));
        }

        Ok(())
    }

    fn report_phase_start<F>(
        &self,
        on_progress: &F,
        cycle: u32,
        total_cycles: u32,
        phase_name: &str,
        phase_num: u8,
    ) where
        F: Fn(ProgressState) + Send + Sync + 'static,
    {
        // Global ETA Calculation
        let total_duration_secs =
            (total_cycles as u64) * 2 * (self.config.phase_duration_ms as u64 / 1000);
        let passed_phases = (cycle * 2) + (phase_num as u32 - 1);
        let passed_seconds = passed_phases as u64 * (self.config.phase_duration_ms as u64 / 1000);
        let remaining = total_duration_secs.saturating_sub(passed_seconds);

        on_progress(ProgressState {
            total_progress: (passed_phases as f32) / ((total_cycles * 2) as f32),
            current_round_progress: 0.0,
            status_message: format!(
                "Cycle {}/{}: Starting Phase {} ({})",
                cycle + 1,
                total_cycles,
                phase_num,
                phase_name
            ),
            eta_seconds: remaining,
            snr_towards: None,
            snr_back: None,
            phase: phase_name.to_string(),
            average_stats: None,
        });
    }

    async fn run_traceroute_phase<F>(
        &mut self,
        rx: &mut meshtastic::packet::PacketReceiver,
        on_progress: &F,
        cycle: u32,
        phase_name: &str,
        phase_num: u8,
        total_cycles: u32,
        is_lna_on: bool,
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

        msnr_log!("Engine started. Config: {:?}", self.config);
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
                        average_stats: None,
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
                             if let Err(e) = self.transport.run_traceroute(&target).await {
                                 msnr_log!("Error sending traceroute: {}", e);
                             }
                         }
                    }
                }
                result = rx.recv() => {
                    match result {
                        Some(packet) => {
                            self.remember_session_key_from_packet(&packet);
                            use meshtastic::protobufs::{PortNum, Data, RouteDiscovery};
                            use meshtastic::protobufs::from_radio::PayloadVariant;
                            use prost::Message;

                            if let Some(PayloadVariant::Packet(mesh_packet)) = packet.payload_variant {
                                if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(Data { portnum, payload, .. })) = mesh_packet.payload_variant {
                                     if portnum == PortNum::TracerouteApp as i32 {
                                         match RouteDiscovery::decode(&payload[..]) {
                                            Ok(route_discovery) => {
                                                msnr_log!("TRACEROUTE RESPONSE RECEIVED! ({})", phase_name);

                                                let snr_towards: Vec<f32> = route_discovery.snr_towards.iter().map(|&x| x as f32 / 4.0).collect();
                                                let snr_back: Vec<f32> = route_discovery.snr_back.iter().map(|&x| x as f32 / 4.0).collect();

                                                 let hit_floor = snr_towards.iter().chain(snr_back.iter()).any(|value| (*value + 32.0).abs() < f32::EPSILON);
                                                 if hit_floor {
                                                     msnr_log!("Skipping traceroute sample (SNR hit -32 dB floor).");
                                                     continue;
                                                 }

                                                let roof_to_mtn_sample = snr_towards.get(1).copied();
                                                let mtn_to_roof_sample = snr_back.get(0).copied();

                                                if matches!(self.config.topology, crate::config::Topology::Relay) {
                                                    let roof_id = match Self::parse_configured_node_u32(&self.config.roof_node_id) {
                                                        Some(id) => id,
                                                        None => {
                                                            msnr_log!("❌ VALIDATION FAIL: Roof node ID is not configured, discarding sample.");
                                                            continue;
                                                        }
                                                    };

                                                    match Self::validate_relay_route(&route_discovery.route, roof_id) {
                                                        Ok(RouteValidationOutcome::RoofOnly) => {
                                                            msnr_log!(
                                                                "✅ VALIDATION PASS: Route reports single-hop via Roof({}), as required.",
                                                                Self::format_node_id(Some(roof_id))
                                                            );
                                                        }
                                                        Err(reason) => {
                                                            msnr_log!(
                                                                "❌ VALIDATION FAIL: {} | Route {}",
                                                                reason,
                                                                Self::format_route(&route_discovery.route)
                                                            );
                                                            continue;
                                                        }
                                                    }
                                                }

                                                if is_lna_on {
                                                    self.stats_lna_on.add_sample(roof_to_mtn_sample, mtn_to_roof_sample);
                                                } else {
                                                    self.stats_lna_off.add_sample(roof_to_mtn_sample, mtn_to_roof_sample);
                                                }

                                                let averages_snapshot = self.current_average_stats();

                                                // Emit live SNR data along with averages
                                                 on_progress(ProgressState {
                                                     total_progress: (cycle as f32) / (total_cycles as f32), // Approximate
                                                     current_round_progress: 0.0, // Don't disrupt progress bar
                                                     status_message: format!("Received Result ({})", phase_name),
                                                     eta_seconds: 0, // Should use tracked value but 0 is fine for ephemeral
                                                     snr_towards: Some(snr_towards.clone()),
                                                     snr_back: Some(snr_back.clone()),
                                                     phase: phase_name.to_string(),
                                                     average_stats: Some(averages_snapshot),
                                                 });

                                                // Logic Validation for Relay Topology
                                                if matches!(self.config.topology, crate::config::Topology::Relay) {
                                                    let record = TracerouteRecord {
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
                                                        msnr_log!("--- SNR DATA (Roof <-> Mtn) ---");
                                                        msnr_log!("Roof -> Mtn : {:.2} dB", snr_towards[1]);
                                                        msnr_log!("Mtn  -> Roof: {:.2} dB", snr_back[0]);
                                                        msnr_log!("-----------------------------");
                                                    }

                                                    if let Err(e) = self.append_csv_record(&record) {
                                                        msnr_log!("Error writing CSV: {}", e);
                                                    } else {
                                                        msnr_log!("Data saved to CSV.");
                                                    }
                                                } else {
                                                    msnr_log!("SNR Towards: {:?}", snr_towards);
                                                    msnr_log!("SNR Back: {:?}", snr_back);
                                                }

                                                 use std::io::Write;
                                                 let _ = std::io::stdout().flush();
                                             }
                                             Err(e) => msnr_log!("Failed to decode RouteDiscovery: {}", e),
                                         }
                                     }
                                }
                            }
                        }
                        None => {
                            msnr_log!("Transport channel closed unexpectedly.");
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn fetch_lora_config_with_retry(
        &mut self,
        rx: &mut meshtastic::packet::PacketReceiver,
        target_node: &str,
        target_id: u32,
        get_req: &AdminMessage,
        attempts: u32,
    ) -> Result<config::LoRaConfig> {
        for attempt in 1..=attempts {
            msnr_log!(
                "Requesting LoRa Config from {}... (Attempt {}/{})",
                target_node,
                attempt,
                attempts
            );
            match self
                .fetch_lora_config_once(rx, target_node, target_id, get_req)
                .await
            {
                Ok(lora) => return Ok(lora),
                Err(e) => {
                    if attempt == attempts {
                        msnr_log!(
                            "WARNING: Get Config failed after {} attempts for {}. {}",
                            attempts,
                            target_node,
                            e
                        );
                        return Err(e);
                    } else {
                        msnr_log!(
                            "Attempt {}/{} failed to fetch config ({}). Retrying...",
                            attempt,
                            attempts,
                            e
                        );
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "Unexpected failure while fetching LoRa config for {}",
            target_node
        ))
    }

    async fn fetch_lora_config_once(
        &mut self,
        rx: &mut meshtastic::packet::PacketReceiver,
        target_node: &str,
        target_id: u32,
        get_req: &AdminMessage,
    ) -> Result<config::LoRaConfig> {
        self.send_admin_with_session(target_node, get_req).await?;
        self.wait_for_lora_config_response(
            rx,
            target_id,
            Duration::from_secs(LNA_WAIT_TIMEOUT_SECS),
        )
        .await
    }

    async fn wait_for_lora_config_response(
        &mut self,
        rx: &mut meshtastic::packet::PacketReceiver,
        target_id: u32,
        timeout: Duration,
    ) -> Result<config::LoRaConfig> {
        use meshtastic::protobufs::{Data, PortNum, from_radio::PayloadVariant, mesh_packet};

        let wait_start = Instant::now();
        loop {
            if wait_start.elapsed() > timeout {
                return Err(anyhow::anyhow!(format!(
                    "Timed out waiting for LoRa config response after {} seconds",
                    timeout.as_secs()
                )));
            }

            let sleep = tokio::time::sleep(Duration::from_millis(100));
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Some(packet) => {
                            self.remember_session_key_from_packet(&packet);
                            if let Some(PayloadVariant::Packet(mesh_packet)) = packet.payload_variant {
                                if mesh_packet.from == target_id {
                                     if let Some(mesh_packet::PayloadVariant::Decoded(Data { portnum, payload, .. })) = mesh_packet.payload_variant {
                                         if portnum == PortNum::AdminApp as i32 {
                                             if let Ok(admin_msg) = AdminMessage::decode(payload.as_slice()) {
                                                 if let Some(admin_message::PayloadVariant::GetConfigResponse(config)) = admin_msg.payload_variant {
                                                     if let Some(config::PayloadVariant::Lora(lora)) = config.payload_variant {
                                                         return Ok(lora);
                                                     }
                                                 }
                                             }
                                         }
                                     }
                                }
                            }
                        }
                        None => {
                            return Err(anyhow::anyhow!(
                                "Transport channel closed while waiting for LoRa config response"
                            ));
                        }
                    }
                }
                _ = sleep => {}
            }
        }
    }

    fn resolve_lna_control_node(&self) -> Option<String> {
        match self.config.topology {
            crate::config::Topology::Relay => match self.config.lna_control_target {
                LnaControlTarget::Disabled => None,
                LnaControlTarget::Roof => self.config.roof_node_id.clone(),
                LnaControlTarget::Mountain => self.config.mountain_node_id.clone(),
            },
            crate::config::Topology::Direct => match self.config.lna_control_target {
                LnaControlTarget::Disabled => None,
                _ => self.config.target_node_id.clone(),
            },
        }
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

    fn log_average_summary(&self) {
        let stats = self.current_average_stats();
        msnr_log!("================ LNA Comparison Summary ================");
        msnr_log!(
            "Samples - LNA OFF: {}, LNA ON: {}",
            stats.lna_off_samples,
            stats.lna_on_samples
        );
        msnr_log!(
            "Roof -> Mountain (avg) | OFF: {} dB | ON: {} dB | Δ: {} dB",
            display_opt(stats.lna_off_roof_to_mtn),
            display_opt(stats.lna_on_roof_to_mtn),
            display_opt(stats.delta_roof_to_mtn())
        );
        msnr_log!(
            "Mountain -> Roof (avg) | OFF: {} dB | ON: {} dB | Δ: {} dB",
            display_opt(stats.lna_off_mtn_to_roof),
            display_opt(stats.lna_on_mtn_to_roof),
            display_opt(stats.delta_mtn_to_roof())
        );
        msnr_log!("========================================================");

        fn display_opt(val: Option<f32>) -> String {
            val.map(|v| format!("{:.2}", v))
                .unwrap_or_else(|| "--".into())
        }
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

    fn current_average_stats(&self) -> AverageStats {
        AverageStats {
            lna_off_samples: self.stats_lna_off.count_roof_to_mtn(),
            lna_off_roof_to_mtn: self.stats_lna_off.average_roof_to_mtn(),
            lna_off_mtn_to_roof: self.stats_lna_off.average_mtn_to_roof(),
            lna_on_samples: self.stats_lna_on.count_roof_to_mtn(),
            lna_on_roof_to_mtn: self.stats_lna_on.average_roof_to_mtn(),
            lna_on_mtn_to_roof: self.stats_lna_on.average_mtn_to_roof(),
        }
    }

    fn validate_relay_route(route: &[u32], roof_id: u32) -> Result<RouteValidationOutcome, String> {
        if route.is_empty() {
            return Err("route metadata is empty".to_string());
        }

        if route.len() != 1 {
            return Err(format!(
                "expected single-hop route via Roof ({}) but received {} hop(s): {}",
                Self::format_node_id(Some(roof_id)),
                route.len(),
                Self::format_route(route)
            ));
        }

        let hop = route[0];
        if hop != roof_id {
            return Err(format!(
                "single-hop route {} does not match configured Roof {}",
                Self::format_node_id(Some(hop)),
                Self::format_node_id(Some(roof_id))
            ));
        }

        Ok(RouteValidationOutcome::RoofOnly)
    }

    fn parse_configured_node_u32(node_id: &Option<String>) -> Option<u32> {
        node_id.as_deref().and_then(Self::parse_node_id_str)
    }

    fn parse_node_id_str(node_id: &str) -> Option<u32> {
        if node_id.is_empty() {
            return None;
        }

        if let Some(stripped) = node_id.strip_prefix('!') {
            u32::from_str_radix(stripped, 16).ok()
        } else if let Some(stripped) = node_id
            .strip_prefix("0x")
            .or_else(|| node_id.strip_prefix("0X"))
        {
            u32::from_str_radix(stripped, 16).ok()
        } else {
            node_id.parse::<u32>().ok()
        }
    }

    fn format_node_id(id: Option<u32>) -> String {
        match id {
            Some(value) => format!("!{:08x}", value),
            None => "unknown".to_string(),
        }
    }

    fn format_route(route: &[u32]) -> String {
        if route.is_empty() {
            return "[]".to_string();
        }
        let parts: Vec<String> = route
            .iter()
            .map(|node| Self::format_node_id(Some(*node)))
            .collect();
        format!("[{}]", parts.join(", "))
    }

    fn normalized_node_id(node_id: &str) -> Option<String> {
        if node_id.is_empty() {
            return None;
        }

        if let Some(id) = node_id.strip_prefix('!') {
            u32::from_str_radix(id, 16)
                .ok()
                .map(|num| format!("!{:08x}", num))
        } else if let Some(id) = node_id
            .strip_prefix("0x")
            .or_else(|| node_id.strip_prefix("0X"))
        {
            u32::from_str_radix(id, 16)
                .ok()
                .map(|num| format!("!{:08x}", num))
        } else if node_id.eq_ignore_ascii_case("broadcast") {
            Some(format!("!{:08x}", u32::MAX))
        } else {
            node_id
                .parse::<u32>()
                .ok()
                .map(|num| format!("!{:08x}", num))
        }
    }

    fn format_node_from_u32(node_num: u32) -> String {
        format!("!{:08x}", node_num)
    }

    fn store_session_key(&mut self, node_num: u32, key: &[u8]) {
        if key.is_empty() {
            return;
        }
        let normalized = Self::format_node_from_u32(node_num);
        if !self.session_keys.contains_key(&normalized) {
            msnr_log!("Stored session key for node {}", normalized);
        }
        self.session_keys.insert(normalized, key.to_vec());
    }

    fn apply_session_key(&self, node_id: &str, msg: &mut AdminMessage) {
        if let Some(normalized) = Self::normalized_node_id(node_id) {
            if let Some(key) = self.session_keys.get(&normalized) {
                msg.session_passkey = key.clone();
            }
        }
    }

    fn remember_session_key_from_packet(&mut self, packet: &meshtastic::protobufs::FromRadio) {
        if let Some(meshtastic::protobufs::from_radio::PayloadVariant::Packet(mesh_packet)) =
            &packet.payload_variant
        {
            if let Some(meshtastic::protobufs::mesh_packet::PayloadVariant::Decoded(ref data)) =
                mesh_packet.payload_variant
            {
                if data.portnum == PortNum::AdminApp as i32 {
                    if let Ok(admin_msg) = AdminMessage::decode(data.payload.as_slice()) {
                        if !admin_msg.session_passkey.is_empty() {
                            self.store_session_key(mesh_packet.from, &admin_msg.session_passkey);
                        }
                    }
                }
            }
        }
    }

    fn has_session_key(&self, node_id: &str) -> bool {
        Self::normalized_node_id(node_id)
            .map(|id| self.session_keys.contains_key(&id))
            .unwrap_or(false)
    }

    async fn send_admin_with_session(
        &mut self,
        target: &str,
        template: &AdminMessage,
    ) -> Result<()> {
        let mut msg = template.clone();
        self.apply_session_key(target, &mut msg);
        self.transport.send_admin(target, msg).await
    }
}
