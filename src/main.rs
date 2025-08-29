extern crate jupiter;

use jupiter::provider::accuweather;
use jupiter::provider::homebrew;
use jupiter::provider::combo;
use jupiter::db_pool;
use jupiter::pool_monitor;
use jupiter::config::Config;
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

    // Load and validate configuration
    let app_config = Config::from_env()
        .map_err(|e| format!("Configuration error: {}", e))?;
    
    app_config.validate()
        .map_err(|e| format!("Configuration validation failed: {}", e))?;
    
    log::info!("Configuration loaded and validated successfully");

    // Acuweather configuration
    let accuweather_config = accuweather::Config{
        apikey: app_config.weather.accu_key.clone(),
        language: None,
        details: None,
        metric: None
    };

    // Homebrew Weather Server configuration (if database config is available)
    let homebrew_config = if let Some(ref db_config) = app_config.homebrew_database {
        let pg = homebrew::PostgresServer::from_config(db_config);
        Some(homebrew::Config{
            apikey: app_config.weather.accu_key.clone(),
            port: 9090,
            pg: pg
        })
    } else {
        log::warn!("Homebrew database configuration not found, skipping homebrew server");
        None
    };

    // Combo server configuration (if database config is available)
    if let Some(ref db_config) = app_config.combo_database {
        let pg = combo::PostgresServer::from_config(db_config);
        let config = combo::Config{
            apikey: app_config.weather.accu_key.clone(),
            port: 9091,
            pg: pg,
            cache_timeout: Some(3600),
            accu_config: Some(accuweather_config),
            homebrew_config: homebrew_config,
            zip_code: app_config.weather.zip_code.clone()
        };

        // Initialize the server
        log::info!("Initializing combo server on port {}", config.port);
        config.init().await
            .map_err(|e| format!("Failed to initialize server: {}", e))?;
        
        // Initialize pool monitors
        pool_monitor::init_monitors().await;
        
        // Start monitoring task (check every 30 seconds)
        pool_monitor::start_monitoring_task(30).await;
        
        log::info!("Server successfully initialized and listening on port {}", config.port);
        log::info!("Pool metrics available at http://localhost:{}/metrics", config.port);
    } else {
        log::error!("Combo database configuration not found - cannot start server");
        return Err("At least one database configuration (combo or homebrew) must be provided".into());
    }

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
        if let Err(e) = signal::ctrl_c().await {
            log::error!("Failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => { signal.recv().await; },
            Err(e) => { log::error!("Failed to install SIGTERM handler: {}", e); }
        }
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
