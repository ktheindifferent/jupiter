use jupiter::provider::{homebrew, combo};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_homebrew_shutdown() {
    // Create a test configuration
    let pg = homebrew::PostgresServer {
        db_name: String::from("test_db"),
        username: String::from("test_user"),
        password: String::from("test_pass"),
        address: String::from("localhost:5432"),
    };
    
    let mut config = homebrew::Config::new(
        String::from("test_api_key"),
        pg,
        9999, // Use a different port for testing
    );
    
    // Initialize the server
    config.init().await;
    
    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Shutdown the server
    let shutdown_result = timeout(
        Duration::from_secs(5),
        config.shutdown()
    ).await;
    
    assert!(shutdown_result.is_ok(), "Shutdown should complete within timeout");
}

#[tokio::test]
async fn test_combo_shutdown() {
    // Create a test configuration
    let pg = combo::PostgresServer {
        db_name: String::from("test_db"),
        username: String::from("test_user"),
        password: String::from("test_pass"),
        address: String::from("localhost:5432"),
    };
    
    let mut config = combo::Config::new(
        None,
        None,
        String::from("test_api_key"),
        Some(3600),
        pg,
        9998, // Use a different port for testing
        String::from("12345"),
    );
    
    // Initialize the server
    config.init().await;
    
    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Shutdown the server
    let shutdown_result = timeout(
        Duration::from_secs(5),
        config.shutdown()
    ).await;
    
    assert!(shutdown_result.is_ok(), "Shutdown should complete within timeout");
}

#[tokio::test]
async fn test_shutdown_with_custom_timeout() {
    let pg = homebrew::PostgresServer {
        db_name: String::from("test_db"),
        username: String::from("test_user"),
        password: String::from("test_pass"),
        address: String::from("localhost:5432"),
    };
    
    let mut config = homebrew::Config::new(
        String::from("test_api_key"),
        pg,
        9997, // Use a different port for testing
    );
    
    // Initialize the server
    config.init().await;
    
    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Shutdown with custom timeout
    let custom_timeout = Duration::from_secs(2);
    let shutdown_result = timeout(
        Duration::from_secs(3),
        config.shutdown_with_timeout(custom_timeout)
    ).await;
    
    assert!(shutdown_result.is_ok(), "Shutdown should complete within custom timeout");
}

#[cfg(unix)]
#[tokio::test]
async fn test_signal_handling() {
    use tokio::signal;
    use std::process;
    
    // Fork a child process to test signal handling
    let pid = process::id();
    
    // Set up signal handler
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .expect("Failed to install SIGTERM handler");
    
    // Send SIGTERM to ourselves in another task
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        unsafe {
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM
            ).expect("Failed to send signal");
        }
    });
    
    // Wait for signal
    let signal_received = timeout(
        Duration::from_secs(1),
        sigterm.recv()
    ).await;
    
    assert!(signal_received.is_ok(), "Should receive SIGTERM signal");
}

#[tokio::test] 
async fn test_concurrent_server_shutdown() {
    // Test shutting down multiple servers concurrently
    let pg1 = homebrew::PostgresServer {
        db_name: String::from("test_db1"),
        username: String::from("test_user"),
        password: String::from("test_pass"),
        address: String::from("localhost:5432"),
    };
    
    let pg2 = combo::PostgresServer {
        db_name: String::from("test_db2"),
        username: String::from("test_user"),
        password: String::from("test_pass"),
        address: String::from("localhost:5432"),
    };
    
    let mut homebrew_config = homebrew::Config::new(
        String::from("test_api_key"),
        pg1,
        9996,
    );
    
    let mut combo_config = combo::Config::new(
        None,
        None,
        String::from("test_api_key"),
        Some(3600),
        pg2,
        9995,
        String::from("12345"),
    );
    
    // Initialize both servers
    homebrew_config.init().await;
    combo_config.init().await;
    
    // Give servers a moment to start
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Shutdown both servers concurrently
    let (result1, result2) = tokio::join!(
        timeout(Duration::from_secs(5), homebrew_config.shutdown()),
        timeout(Duration::from_secs(5), combo_config.shutdown())
    );
    
    assert!(result1.is_ok(), "Homebrew shutdown should complete");
    assert!(result2.is_ok(), "Combo shutdown should complete");
}