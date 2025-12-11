pub mod config;
pub mod engine;
pub mod logging;
pub mod transport;

pub use config::{Config, TransportMode};
pub use engine::{Engine, ProgressState};
pub use logging::{clear_log_callback, set_log_callback};
pub use transport::{IpTransport, SerialTransport, Transport};

#[macro_export]
macro_rules! msnr_log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("{}", msg);
        $crate::logging::emit(&msg);
    }};
}

#[macro_export]
macro_rules! msnr_log_err {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        eprintln!("{}", msg);
        $crate::logging::emit(&msg);
    }};
}
