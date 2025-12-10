import React from 'react';
import { ProgressState } from '../types';

interface Props {
    progress: ProgressState | null;
    logs: string[];
}

export const Dashboard: React.FC<Props> = ({ progress, logs }) => {
    return (
        <div className="glass-panel main-content">
            <h2>Dashboard</h2>

            {progress && (
                <div className="progress-container">
                    <div className="status-header">
                        <h3>Status: <span className="status-badge">{progress.status_message}</span></h3>
                        <span>ETA: {progress.eta_seconds}s</span>
                    </div>

                    <label>Total Progress</label>
                    <div className="progress-bar">
                        <div
                            className="progress-fill"
                            style={{ width: `${progress.total_progress * 100}%` }}
                        />
                    </div>

                    <label>Current Round</label>
                    <div className="progress-bar">
                        <div
                            className="progress-fill"
                            style={{ width: `${progress.current_round_progress * 100}%` }}
                        />
                    </div>
                </div>
            )}

            <h3>Logs</h3>
            <div className="log-output">
                {logs.map((log, i) => (
                    <div key={i}>{log}</div>
                ))}
            </div>
        </div>
    );
};
