use anyhow::Result;
use clap::{Parser, Subcommand};
use msnr_core::{Config, Engine, IpTransport, SerialTransport, Transport, TransportMode};


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the test engine
    Run {
        /// Transport mode (ip or serial)
        #[arg(long, default_value = "ip")]
        transport: String,

        /// IP address (for ip mode)
        #[arg(long, default_value = "192.168.1.100")]
        ip: String,

        /// Port (for ip mode)
        #[arg(long, default_value_t = 4403)]
        port: u16,

        /// Serial port (for serial mode)
        #[arg(long)]
        serial: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Run { transport, ip, port, serial }) => {
            println!("Starting MSNR Tool CLI...");

            let mut config = Config::default();
            
            let transport_impl: Box<dyn Transport> = match transport.as_str() {
                "serial" => {
                    config.transport_mode = TransportMode::Serial;
                    if let Some(s) = serial {
                        config.serial_port = Some(s.clone());
                        Box::new(SerialTransport::new(s.clone()))
                    } else {
                        eprintln!("Error: --serial is required for serial transport");
                        return Ok(());
                    }
                }
                _ => {
                    config.transport_mode = TransportMode::Ip;
                    config.ip = Some(ip.clone());
                    config.port = Some(*port);
                    Box::new(IpTransport::new(ip.clone(), *port))
                }
            };

            let mut engine = Engine::new(config, transport_impl);

            engine.run(|progress| {
                // Clear line and print progress
                print!("\r[{}] {:.1}% | {}", 
                    progress_bar(progress.total_progress), 
                    progress.total_progress * 100.0,
                    progress.status_message
                );
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }).await?;
            
            println!("\nTest completed!");
        }
        None => {
            println!("No command specified. Use --help for usage.");
        }
    }

    Ok(())
}

fn progress_bar(progress: f32) -> String {
    let width = 20;
    let filled = (progress * width as f32) as usize;
    let empty = width - filled;
    format!("{}{}", "#".repeat(filled), "-".repeat(empty))
}
