use msnr_core::{
    set_log_callback, Config, Engine, IpTransport, SerialTransport, Transport, TransportMode,
};
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::Mutex as AsyncMutex;

struct AppState {
    engine_handle: Arc<AsyncMutex<Option<tokio::task::JoinHandle<()>>>>,
}

#[tauri::command]
fn get_serial_ports() -> Vec<String> {
    serialport::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .unwrap_or_default()
}

#[tauri::command]
async fn start_test(
    config: Config,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let mut handle_guard = state.engine_handle.lock().await;

    // Check if there is an active handle
    if let Some(handle) = handle_guard.as_ref() {
        if !handle.is_finished() {
            return Err("Test already running".to_string());
        }
    }

    let transport_impl: Box<dyn Transport> = match config.transport_mode {
        TransportMode::Serial => {
            if let Some(s) = &config.serial_port {
                Box::new(SerialTransport::new(s.clone()))
            } else {
                return Err("Serial port not specified".to_string());
            }
        }
        TransportMode::Ip => {
            let ip = config.ip.clone().unwrap_or("127.0.0.1".to_string());
            let port = config.port.unwrap_or(4403);
            Box::new(IpTransport::new(ip, port))
        }
    };

    {
        let console_handle = app_handle.clone();
        set_log_callback(Arc::new(move |line| {
            let _ = console_handle.emit("console-log", line.clone());
        }));
    }

    let mut engine = Engine::new(config, transport_impl);
    let app_handle_clone = app_handle.clone();

    let handle = tokio::spawn(async move {
        if let Err(e) = engine
            .run(move |progress| {
                let _ = app_handle_clone.emit("test-progress", progress);
            })
            .await
        {
            let _ = app_handle.emit("test-error", e.to_string());
        }
        let _ = app_handle.emit("test-complete", ());
    });

    *handle_guard = Some(handle);
    Ok(())
}

#[tauri::command]
async fn stop_test(state: State<'_, AppState>) -> Result<(), String> {
    let mut handle_guard = state.engine_handle.lock().await;
    if let Some(handle) = handle_guard.take() {
        handle.abort();
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            engine_handle: Arc::new(AsyncMutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            get_serial_ports,
            start_test,
            stop_test
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
