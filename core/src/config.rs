use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransportMode {
    Ip,
    Serial,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Topology {
    Relay,
    Direct,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelayTestMode {
    RoofOnly,
    MountainOnly,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DirectTestMode {
    LocalLna,
    TargetLna,
    Both,
    ScanOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TestMode {
    Relay(RelayTestMode),
    Direct(DirectTestMode),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OutputFormat {
    Csv,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // Connection
    pub transport_mode: TransportMode,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub serial_port: Option<String>,

    // Topology & Test Mode
    pub topology: Topology,
    pub test_mode: TestMode,

    // Test Parameters
    pub interval_ms: u64,
    pub phase_duration_ms: u64,
    pub cycles: u32,
    pub scan_duration_ms: Option<u64>,

    // Node IDs
    pub local_node_id: Option<String>,
    pub roof_node_id: Option<String>,
    pub mountain_node_id: Option<String>,
    pub target_node_id: Option<String>,

    // Output
    pub output_path: String,
    pub output_format: OutputFormat,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            transport_mode: TransportMode::Ip,
            ip: Some("192.168.1.100".to_string()),
            port: Some(4403),
            serial_port: None,
            topology: Topology::Relay,
            test_mode: TestMode::Relay(RelayTestMode::RoofOnly),
            interval_ms: 30000,
            phase_duration_ms: 450000, // Default 7.5 minutes per phase (15 minutes per cycle)
            cycles: 2,
            scan_duration_ms: None,
            local_node_id: None,
            roof_node_id: None,
            mountain_node_id: None,
            target_node_id: None,
            output_path: "results.csv".to_string(),
            output_format: OutputFormat::Csv,
        }
    }
}
