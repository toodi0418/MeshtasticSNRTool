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
}

export interface ProgressState {
    total_progress: number;
    current_round_progress: number;
    status_message: string;
    eta_seconds: number;
}
