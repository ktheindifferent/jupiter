extern crate jupiter;

use jupiter::provider::accuweather;
use jupiter::provider::homebrew;
use jupiter::provider::combo;
use std::env;
use tokio::signal;

// store application version as a const
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    simple_logger::init_with_level(log::Level::Info).unwrap_or_else(|e| {
        eprintln!("Failed to initialize logger: {}", e);
    });

    log::info!("Starting Jupiter Weather Server v{}", VERSION.unwrap_or("unknown"));

    // Load environment variables with proper error handling
    let accu_key = env::var("ACCUWEATHERKEY")
        .map_err(|_| "Environment variable ACCUWEATHERKEY is not set")?;
    let zip_code = env::var("ZIP_CODE")
        .map_err(|_| "Environment variable ZIP_CODE is not set")?;

    // Acuweather configuration
    let accuweather_config = accuweather::Config{
        apikey: String::from(accu_key.clone()),
        language: None,
        details: None,
        metric: None
    };

    // Homebrew Weather Server configuration
    let pg = homebrew::PostgresServer::new();
    let mut homebrew_config = homebrew::Config::new(
        String::from(accu_key.clone()),
        pg,
        9090
    );

    // Combo server configuration
    let pg = combo::PostgresServer::new();
    let mut config = combo::Config::new(
        Some(accuweather_config),
        Some(homebrew_config.clone()),
        String::from(accu_key.clone()),
        Some(3600),
        pg,
        9091,
        String::from(zip_code)
    );

    // Initialize the server
    log::info!("Initializing combo server on port {}", config.port);
    config.init().await;
    log::info!("Server successfully initialized and listening on port {}", config.port);

    // Wait for shutdown signal
    shutdown_signal().await;
    
    log::info!("Shutdown signal received, gracefully shutting down...");
    
    // Shutdown all servers gracefully
    config.shutdown().await;
    
    log::info!("Server shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(unix)]
    let hangup = async {
        signal::unix::signal(signal::unix::SignalKind::hangup())
            .expect("failed to install SIGHUP handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    
    #[cfg(not(unix))]
    let hangup = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            log::info!("Received Ctrl+C (SIGINT) signal");
        },
        _ = terminate => {
            log::info!("Received SIGTERM signal");
        },
        _ = hangup => {
            log::info!("Received SIGHUP signal");
        },
    }
}
