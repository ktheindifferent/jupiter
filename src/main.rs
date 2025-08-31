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
    let mut homebrew_config = if let Some(ref db_config) = app_config.homebrew_database {
        let pg = homebrew::PostgresServer::from_config(db_config);
        Some(homebrew::Config::new(
            app_config.weather.accu_key.clone(),
            pg,
            9090
        ))
    } else {
        log::warn!("Homebrew database configuration not found, skipping homebrew server");
        None
    };

    // Initialize homebrew server if configured
    if let Some(ref mut hb_config) = homebrew_config {
        hb_config.init().await
            .map_err(|e| format!("Failed to initialize homebrew server: {}", e))?;
        log::info!("Homebrew server initialized on port {}", hb_config.port);
    }

    // Combo server configuration (if database config is available)
    let mut combo_config = if let Some(ref db_config) = app_config.combo_database {
        let pg = combo::PostgresServer::from_config(db_config);
        Some(combo::Config::new(
            Some(accuweather_config),
            homebrew_config.clone(),
            app_config.weather.accu_key.clone(),
            Some(3600),
            pg,
            9091,
            app_config.weather.zip_code.clone()
        ))
    } else {
        log::error!("Combo database configuration not found - cannot start server");
        return Err("At least one database configuration (combo or homebrew) must be provided".into());
    };

    // Initialize combo server
    if let Some(ref mut config) = combo_config {
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
    }

    // Wait for shutdown signal
    shutdown_signal().await;
    
    log::info!("Shutdown signal received, gracefully shutting down...");
    
    // Shutdown all servers gracefully
    if let Some(ref mut config) = combo_config {
        config.shutdown().await;
    }
    if let Some(ref mut hb_config) = homebrew_config {
        hb_config.shutdown().await;
    }
    
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

    #[cfg(unix)]
    let hangup = async {
        match signal::unix::signal(signal::unix::SignalKind::hangup()) {
            Ok(mut signal) => { signal.recv().await; },
            Err(e) => { log::error!("Failed to install SIGHUP handler: {}", e); }
        }
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
