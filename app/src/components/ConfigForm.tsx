import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Config } from '../types';

interface Props {
    config: Config;
    setConfig: (config: Config) => void;
    isRunning: boolean;
    onStart: () => void;
    onStop: () => void;
}

export const ConfigForm: React.FC<Props> = ({ config, setConfig, isRunning, onStart, onStop }) => {
    const [serialPorts, setSerialPorts] = useState<string[]>([]);

    useEffect(() => {
        invoke<string[]>('get_serial_ports').then(setSerialPorts).catch(console.error);
    }, []);

    const handleChange = (field: keyof Config, value: any) => {
        setConfig({ ...config, [field]: value });
    };

    const intervalSeconds = Math.round((config.interval_ms || 0) / 1000);
    const phaseSeconds = Math.round((config.phase_duration_ms || 0) / 1000);

    const handleIntervalSecondsChange = (value: number) => {
        const sanitized = Math.max(0, value);
        setConfig({ ...config, interval_ms: sanitized * 1000 });
    };

    const handlePhaseSecondsChange = (value: number) => {
        const sanitized = Math.max(0, value);
        setConfig({ ...config, phase_duration_ms: sanitized * 1000 });
    };

    return (
        <div className="glass sidebar">
            <h2>Configuration</h2>

            <div className="form-group">
                <label>Transport Mode</label>
                <select
                    value={config.transport_mode}
                    onChange={(e) => handleChange('transport_mode', e.target.value)}
                    disabled={isRunning}
                >
                    <option value="Ip">IP Network</option>
                    <option value="Serial">Serial Port</option>
                </select>
            </div>

            {config.transport_mode === 'Ip' ? (
                <>
                    <div className="form-group">
                        <label>IP Address</label>
                        <input
                            type="text"
                            value={config.ip || ''}
                            onChange={(e) => handleChange('ip', e.target.value)}
                            disabled={isRunning}
                        />
                    </div>
                    <div className="form-group">
                        <label>Port</label>
                        <input
                            type="number"
                            value={config.port || 4403}
                            onChange={(e) => handleChange('port', parseInt(e.target.value))}
                            disabled={isRunning}
                        />
                    </div>
                </>
            ) : (
                <div className="form-group">
                    <label>Serial Port</label>
                    <select
                        value={config.serial_port || ''}
                        onChange={(e) => handleChange('serial_port', e.target.value)}
                        disabled={isRunning}
                    >
                        <option value="">Select Port</option>
                        {serialPorts.map(p => <option key={p} value={p}>{p}</option>)}
                    </select>
                </div>
            )}

            <div className="form-group">
                <label>Topology</label>
                <select
                    value={config.topology}
                    onChange={(e) => handleChange('topology', e.target.value)}
                    disabled={isRunning}
                >
                    <option value="Relay">Relay (Roof -&gt; Mountain)</option>
                    <option value="Direct">Direct (Local -&gt; Target)</option>
                </select>
            </div>

            <div className="form-group">
                <label>Interval (秒)</label>
                <input
                    type="number"
                    value={intervalSeconds}
                    min={0}
                    onChange={(e) => handleIntervalSecondsChange(parseInt(e.target.value) || 0)}
                    disabled={isRunning}
                />
            </div>

            <div className="form-group">
                <label>Cycle 時間 (秒)</label>
                <input
                    type="number"
                    value={phaseSeconds}
                    min={0}
                    onChange={(e) => handlePhaseSecondsChange(parseInt(e.target.value) || 0)}
                    disabled={isRunning}
                />
            </div>

            {config.topology === 'Relay' && (
                <>
                    <div className="form-group">
                        <label>Roof Node ID (Relay 1)</label>
                        <input
                            type="text"
                            value={config.roof_node_id || ''}
                            onChange={(e) => handleChange('roof_node_id', e.target.value)}
                            placeholder="e.g. !867263da"
                            disabled={isRunning}
                        />
                    </div>
                    <div className="form-group">
                        <label>Mountain Node ID (Relay 2)</label>
                        <input
                            type="text"
                            value={config.mountain_node_id || ''}
                            onChange={(e) => handleChange('mountain_node_id', e.target.value)}
                            placeholder="e.g. !550d885b"
                            disabled={isRunning}
                        />
                    </div>
                </>
            )}

            {config.topology === 'Direct' && (
                <div className="form-group">
                    <label>Target Node ID</label>
                    <input
                        type="text"
                        value={config.target_node_id || ''}
                        onChange={(e) => handleChange('target_node_id', e.target.value)}
                        placeholder="e.g. !12345678"
                        disabled={isRunning}
                    />
                </div>
            )}

            <button onClick={isRunning ? onStop : onStart} disabled={false}>
                {isRunning ? 'Stop Test' : 'Start Test'}
            </button>
        </div>
    );
};
