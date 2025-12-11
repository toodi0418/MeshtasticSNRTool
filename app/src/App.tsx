import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './App.css';
import { ConfigForm } from './components/ConfigForm';
import { Dashboard } from './components/Dashboard';
import { ResultModal } from './components/ResultModal';
import { Config, ProgressState, AverageStats } from './types';

const createEmptyProgress = (): ProgressState => ({
  total_progress: 0,
  current_round_progress: 0,
  status_message: 'Idle',
  eta_seconds: 0,
  snr_towards: undefined,
  snr_back: undefined,
  phase: 'Idle',
  average_stats: undefined,
});

function App() {

  const [config, setConfig] = useState<Config>({
    transport_mode: 'Ip',
    ip: '172.16.8.92',
    port: 4403,
    topology: 'Relay',
    test_mode: { Relay: 'RoofOnly' },
    interval_ms: 30000,
    phase_duration_ms: 450000,
    cycles: 2,
    output_path: 'results.csv',
    output_format: 'Csv',
    roof_node_id: '!867263da',
    mountain_node_id: '!550d885b',
    target_node_id: '' // User can fill this
  });

  const [isRunning, setIsRunning] = useState(false);
  const [progress, setProgress] = useState<ProgressState>(() => createEmptyProgress());
  const [logs, setLogs] = useState<string[]>([]);
  const [resetToken, setResetToken] = useState(0);
  const [summaryStats, setSummaryStats] = useState<AverageStats | null>(null);
  const [showSummary, setShowSummary] = useState(false);
  const resetProgress = () => setProgress(createEmptyProgress());

  useEffect(() => {
    const unlisten = listen<ProgressState>('test-progress', (event) => {
      setProgress(event.payload);
      if (event.payload.phase === 'Done' && event.payload.average_stats) {
        setSummaryStats(event.payload.average_stats);
        setShowSummary(true);
      }
    });

    const unlistenComplete = listen('test-complete', () => {
      setIsRunning(false);
      setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Test Completed`]);
    });

    const unlistenError = listen<string>('test-error', (event) => {
      setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Error: ${event.payload}`]);
    });

    const unlistenConsole = listen<string>('console-log', (event) => {
      setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] ${event.payload}`].slice(-100));
    });

    return () => {
      unlisten.then(f => f());
      unlistenComplete.then(f => f());
      unlistenError.then(f => f());
      unlistenConsole.then(f => f());
    };
  }, []);

  const handleStart = async () => {
    try {
      setResetToken((token) => token + 1);
      resetProgress();
      setSummaryStats(null);
      setShowSummary(false);
      await invoke('start_test', { config });
      setIsRunning(true);
      setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Test Started`]);
    } catch (e) {
      console.error(e);
      setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Error: ${e}`]);
    }
  };

  const handleStop = async () => {
    try {
      await invoke('stop_test');
      setIsRunning(false);
      setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Test Stopped`]);
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <div className="app-container">
      <ConfigForm
        config={config}
        setConfig={setConfig}
        isRunning={isRunning}
        onStart={handleStart}
        onStop={handleStop}
      />
      <Dashboard progress={progress} logs={logs} resetToken={resetToken} />
      {showSummary && summaryStats && (
        <ResultModal stats={summaryStats} onClose={() => setShowSummary(false)} />
      )}
    </div>
  );
}

export default App;
