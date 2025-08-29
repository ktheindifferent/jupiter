extern crate jupiter;

use jupiter::provider::accuweather;
use jupiter::provider::homebrew;
use jupiter::provider::combo;
use jupiter::db_pool;
use jupiter::pool_monitor;
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
    let homebrew_config = homebrew::Config{
        apikey: String::from(accu_key.clone()),
        port: 9090,
        pg: pg
    };

    // Combo server configuration
    let pg = combo::PostgresServer::new();
    let config = combo::Config{
        apikey: String::from(accu_key.clone()),
        port: 9091,
        pg: pg,
        cache_timeout: Some(3600),
        accu_config: Some(accuweather_config),
        homebrew_config: Some(homebrew_config),
        zip_code: String::from(zip_code)
    };

    // Initialize the server
    log::info!("Initializing combo server on port {}", config.port);
    config.init().await;
    
    // Initialize pool monitors
    pool_monitor::init_monitors().await;
    
    // Start monitoring task (check every 30 seconds)
    pool_monitor::start_monitoring_task(30).await;
    
    log::info!("Server successfully initialized and listening on port {}", config.port);
    log::info!("Pool metrics available at http://localhost:{}/metrics", config.port);

    // Wait for shutdown signal
    shutdown_signal().await;
    
    log::info!("Shutdown signal received, gracefully shutting down...");
    
    // Shutdown database connection pools
    db_pool::shutdown_pools().await;
    
    // Give the server threads a moment to finish current requests
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
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

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            log::info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            log::info!("Received SIGTERM signal");
        },
    }
}
