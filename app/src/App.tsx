import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './App.css';
import { ConfigForm } from './components/ConfigForm';
import { Dashboard } from './components/Dashboard';
import { Config, ProgressState } from './types';

function App() {
  const [config, setConfig] = useState<Config>({
    transport_mode: 'Ip',
    ip: '127.0.0.1',
    port: 4403,
    topology: 'Relay',
    test_mode: { Relay: 'RoofOnly' },
    interval_ms: 1000,
    phase_duration_ms: 60000,
    cycles: 2,
    output_path: 'results.csv',
    output_format: 'Csv'
  });

  const [isRunning, setIsRunning] = useState(false);
  const [progress, setProgress] = useState<ProgressState | null>(null);
  const [logs, setLogs] = useState<string[]>([]);

  useEffect(() => {
    const unlisten = listen<ProgressState>('test-progress', (event) => {
      setProgress(event.payload);
      setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] ${event.payload.status_message}`].slice(-100));
    });

    const unlistenComplete = listen('test-complete', () => {
      setIsRunning(false);
      setLogs(prev => [...prev, `[${new Date().toLocaleTimeString()}] Test Completed`]);
    });

    return () => {
      unlisten.then(f => f());
      unlistenComplete.then(f => f());
    };
  }, []);

  const handleStart = async () => {
    try {
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
      <Dashboard progress={progress} logs={logs} />
    </div>
  );
}

export default App;
