export interface Config {
    transport_mode: 'Ip' | 'Serial';
    ip?: string;
    port?: number;
    serial_port?: string;
    topology: 'Relay' | 'Direct';
    test_mode: any; // Simplified for now
    interval_ms: number;
    phase_duration_ms: number;
    cycles: number;
    output_path: string;
    output_format: 'Csv' | 'Json';
    target_node_id?: string;
    local_node_id?: string;
    roof_node_id?: string;
    mountain_node_id?: string;
}

export interface ProgressState {
    total_progress: number;
    current_round_progress: number;
    status_message: string;
    eta_seconds: number;
    snr_towards?: number[];
    snr_back?: number[];
    phase?: string;
    average_stats?: AverageStats;
}

export interface AverageStats {
    lna_off_samples: number;
    lna_off_roof_to_mtn?: number | null;
    lna_off_mtn_to_roof?: number | null;
    lna_on_samples: number;
    lna_on_roof_to_mtn?: number | null;
    lna_on_mtn_to_roof?: number | null;
}
