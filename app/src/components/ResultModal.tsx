import React from 'react';
import { AverageStats } from '../types';

interface ResultModalProps {
    stats: AverageStats;
    onClose: () => void;
}

const hasValue = (val?: number | null): val is number =>
    val !== undefined && val !== null && !Number.isNaN(val);

const formatDb = (val?: number | null) => {
    if (!hasValue(val)) return '--';
    return `${val.toFixed(2)} dB`;
};

const computeDelta = (on?: number | null, off?: number | null) => {
    if (!hasValue(on) || !hasValue(off)) return undefined;
    return on - off;
};

export const ResultModal: React.FC<ResultModalProps> = ({ stats, onClose }) => {
    const roofDelta = computeDelta(stats.lna_on_roof_to_mtn, stats.lna_off_roof_to_mtn);
    const mtnDelta = computeDelta(stats.lna_on_mtn_to_roof, stats.lna_off_mtn_to_roof);

    return (
        <div className="modal-backdrop">
            <div className="result-modal glass">
                <h2>測試結算</h2>
                <p className="modal-subtitle">LNA ON/OFF 平均值與差值</p>
                <div className="modal-grid">
                    <div>
                        <h4>LNA OFF（{stats.lna_off_samples} 筆）</h4>
                        <div>Roof → Mtn: {formatDb(stats.lna_off_roof_to_mtn)}</div>
                        <div>Mtn → Roof: {formatDb(stats.lna_off_mtn_to_roof)}</div>
                    </div>
                    <div>
                        <h4>LNA ON（{stats.lna_on_samples} 筆）</h4>
                        <div>Roof → Mtn: {formatDb(stats.lna_on_roof_to_mtn)}</div>
                        <div>Mtn → Roof: {formatDb(stats.lna_on_mtn_to_roof)}</div>
                    </div>
                    <div className="delta-card">
                        <h4>差值 (ON - OFF)</h4>
                        <div>Roof → Mtn: {formatDb(roofDelta)}</div>
                        <div>Mtn → Roof: {formatDb(mtnDelta)}</div>
                    </div>
                </div>
                <div className="modal-actions">
                    <button onClick={onClose}>關閉</button>
                </div>
            </div>
        </div>
    );
};
