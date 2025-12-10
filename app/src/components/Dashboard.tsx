import React, { useState, useEffect } from 'react';
import { ProgressState } from '../types';
import { SignalChart, SignalData } from './SignalChart';
import { Activity, Clock, Terminal } from 'lucide-react';

interface Props {
    progress: ProgressState;
    logs: string[];
    resetToken: number;
}

export const Dashboard: React.FC<Props> = ({ progress, logs, resetToken }) => {
    const [history, setHistory] = useState<SignalData[]>([]);

    useEffect(() => {
        const snrTowards = progress.snr_towards;
        const snrBack = progress.snr_back;
        if (snrTowards && snrBack) {
            setHistory(prev => [
                ...prev,
                {
                    time: new Date().toLocaleTimeString('en-US', { hour12: false }),
                    snr_towards: snrTowards[1] ?? 0, // Roof -> Mtn
                    snr_back: snrBack[0] ?? 0,    // Mtn -> Roof
                    phase: progress.phase || 'Unknown'
                }
            ].slice(-50));
        }
    }, [progress]);

    useEffect(() => {
        setHistory([]);
    }, [resetToken]);

    const formatTime = (secs: number) => {
        const m = Math.floor(secs / 60);
        const s = secs % 60;
        return `${m}m ${s.toString().padStart(2, '0')}s`;
    };

    const hasValue = (val: number | null | undefined): val is number =>
        val !== undefined && val !== null && !Number.isNaN(val);

    const formatDb = (val?: number | null) => {
        if (!hasValue(val)) return '--';
        return `${val.toFixed(2)} dB`;
    };

    const averages = progress.average_stats;
    const roofDelta =
        hasValue(averages?.lna_on_roof_to_mtn) && hasValue(averages?.lna_off_roof_to_mtn)
            ? averages!.lna_on_roof_to_mtn! - averages!.lna_off_roof_to_mtn!
            : undefined;
    const mtnDelta =
        hasValue(averages?.lna_on_mtn_to_roof) && hasValue(averages?.lna_off_mtn_to_roof)
            ? averages!.lna_on_mtn_to_roof! - averages!.lna_off_mtn_to_roof!
            : undefined;
    const showAverageStats =
        progress.phase === 'Done' &&
        (averages?.lna_off_samples ?? 0) > 0 &&
        (averages?.lna_on_samples ?? 0) > 0;

    return (
        <div className="dashboard-container">
            {/* Top Stats Row */}
            <div className="stats-row">
                <div className="stat-card glass">
                    <div className="stat-icon"><Activity size={24} color="#3b82f6" /></div>
                    <div className="stat-content">
                        <label>Current Phase</label>
                        <div className="value highlight">{progress.phase || 'Idle'}</div>
                        <div className="sub-value">{progress.status_message || 'Waiting to start...'}</div>
                    </div>
                </div>

                <div className="stat-card glass">
                    <div className="stat-icon"><Clock size={24} color="#10b981" /></div>
                    <div className="stat-content">
                        <label>Total Time Remaining</label>
                        <div className="value">{formatTime(progress.eta_seconds)}</div>
                        <div className="sub-value">
                            Progress: {Math.round(progress.total_progress * 100)}%
                        </div>
                    </div>
                    <div className="mini-progress-bar">
                        <div className="fill" style={{ width: `${Math.min(progress.total_progress, 1) * 100}%` }}></div>
                    </div>
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

            {/* Averages Section */}
            {showAverageStats && (
                <div className="chart-section glass">
                    <div className="section-header">
                        <h3>LNA 平均值比較</h3>
                    </div>
                    <div className="avg-grid">
                        <div className="avg-card">
                            <h4>LNA OFF（{averages?.lna_off_samples ?? 0} 筆）</h4>
                            <div>Roof → Mtn: {formatDb(averages?.lna_off_roof_to_mtn)}</div>
                            <div>Mtn → Roof: {formatDb(averages?.lna_off_mtn_to_roof)}</div>
                        </div>
                        <div className="avg-card">
                            <h4>LNA ON（{averages?.lna_on_samples ?? 0} 筆）</h4>
                            <div>Roof → Mtn: {formatDb(averages?.lna_on_roof_to_mtn)}</div>
                            <div>Mtn → Roof: {formatDb(averages?.lna_on_mtn_to_roof)}</div>
                        </div>
                        <div className="avg-card">
                            <h4>差值 (ON - OFF)</h4>
                            <div>Roof → Mtn: {formatDb(roofDelta)}</div>
                            <div>Mtn → Roof: {formatDb(mtnDelta)}</div>
                        </div>
                    </div>
                </div>
            )}

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
