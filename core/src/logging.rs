use std::sync::{Arc, Mutex, OnceLock};

pub type LogCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

fn log_storage() -> &'static Mutex<Option<LogCallback>> {
    static STORE: OnceLock<Mutex<Option<LogCallback>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
}

pub fn set_log_callback(callback: LogCallback) {
    let mut guard = log_storage().lock().unwrap();
    *guard = Some(callback);
}

pub fn clear_log_callback() {
    let mut guard = log_storage().lock().unwrap();
    *guard = None;
}

pub(crate) fn emit(message: &str) {
    if let Some(cb) = log_storage().lock().unwrap().as_ref() {
        cb(message.to_string());
    }
}
