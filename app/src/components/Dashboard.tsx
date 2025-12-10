import React, { useState, useEffect } from 'react';
import { ProgressState } from '../types';
import { SignalChart, SignalData } from './SignalChart';
import { Activity, Clock, Terminal } from 'lucide-react';

interface Props {
    progress: ProgressState | null;
    logs: string[];
}

export const Dashboard: React.FC<Props> = ({ progress, logs }) => {
    const [history, setHistory] = useState<SignalData[]>([]);

    useEffect(() => {
        if (progress?.snr_towards && progress?.snr_back) {
            setHistory(prev => [
                ...prev,
                {
                    time: new Date().toLocaleTimeString('en-US', { hour12: false }),
                    snr_towards: progress.snr_towards![1] ?? 0, // Roof -> Mtn
                    snr_back: progress.snr_back![0] ?? 0,    // Mtn -> Roof
                    phase: progress.phase || 'Unknown'
                }
            ].slice(-50));
        }
    }, [progress]);

    const formatTime = (secs: number) => {
        const m = Math.floor(secs / 60);
        const s = secs % 60;
        return `${m}m ${s.toString().padStart(2, '0')}s`;
    };

    return (
        <div className="dashboard-container">
            {/* Top Stats Row */}
            <div className="stats-row">
                <div className="stat-card glass">
                    <div className="stat-icon"><Activity size={24} color="#3b82f6" /></div>
                    <div className="stat-content">
                        <label>Current Phase</label>
                        <div className="value highlight">{progress?.phase || 'Idle'}</div>
                        <div className="sub-value">{progress?.status_message || 'Waiting to start...'}</div>
                    </div>
                </div>

                <div className="stat-card glass">
                    <div className="stat-icon"><Clock size={24} color="#10b981" /></div>
                    <div className="stat-content">
                        <label>Total Time Remaining</label>
                        <div className="value">{progress ? formatTime(progress.eta_seconds) : '--:--'}</div>
                        <div className="sub-value">
                            Progress: {progress ? Math.round(progress.total_progress * 100) : 0}%
                        </div>
                    </div>
                    {progress && (
                        <div className="mini-progress-bar">
                            <div className="fill" style={{ width: `${progress.total_progress * 100}%` }}></div>
                        </div>
                    )}
                </div>
            </div>

            {/* Chart Section */}
            <div className="chart-section glass">
                <div className="section-header">
                    <h3>Real-time SNR Analysis (Roof ↔ Mountain)</h3>
                </div>
                <div className="chart-wrapper">
                    <SignalChart data={history} />
                </div>
            </div>

            {/* Logs Section */}
            <div className="logs-section glass">
                <div className="section-header">
                    <Terminal size={18} /> <h3>System Logs</h3>
                </div>
                <div className="log-window">
                    {logs.map((log, i) => (
                        <div key={i} className="log-entry">
                            <span className="log-time">{log.substring(0, log.indexOf(']') + 1)}</span>
                            <span className="log-msg">{log.substring(log.indexOf(']') + 1)}</span>
                        </div>
                    ))}
                    {logs.length === 0 && <div className="log-entry empty">No logs yet...</div>}
                </div>
            </div>
        </div>
    );
};
