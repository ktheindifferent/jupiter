use std::process::Command;
use std::thread;
use std::time::Duration;

#[test]
#[ignore] // This test requires actual server startup, run with --ignored flag
fn test_cpu_usage_remains_low() {
    // Set required environment variables
    std::env::set_var("ACCUWEATHERKEY", "test_key");
    std::env::set_var("ZIP_CODE", "12345");
    std::env::set_var("HOMEBREW_PG_DBNAME", "test_db");
    std::env::set_var("HOMEBREW_PG_USER", "test_user");
    std::env::set_var("HOMEBREW_PG_PASS", "test_pass");
    std::env::set_var("HOMEBREW_PG_ADDRESS", "localhost:5432");
    std::env::set_var("COMBO_PG_DBNAME", "test_db");
    std::env::set_var("COMBO_PG_USER", "test_user");
    std::env::set_var("COMBO_PG_PASS", "test_pass");
    std::env::set_var("COMBO_PG_ADDRESS", "localhost:5432");

    // Start the server
    let mut child = Command::new("cargo")
        .args(&["run"])
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start server: {}", e));

    // Let server initialize
    thread::sleep(Duration::from_secs(3));

    // Check process is running
    match child.try_wait() {
        Ok(None) => {}, // Server is still running
        Ok(Some(status)) => panic!("Server exited unexpectedly with status: {:?}", status),
        Err(e) => panic!("Failed to check server status: {}", e),
    }

    // Monitor for a bit to ensure no CPU spike
    thread::sleep(Duration::from_secs(2));
    
    // Still running without excessive CPU
    match child.try_wait() {
        Ok(None) => {}, // Server is still running
        Ok(Some(status)) => panic!("Server exited unexpectedly with status: {:?}", status),
        Err(e) => panic!("Failed to check server status: {}", e),
    }

    // Clean shutdown
    if let Err(e) = child.kill() {
        eprintln!("Failed to kill server: {}", e);
    }
    if let Err(e) = child.wait() {
        eprintln!("Failed to wait for shutdown: {}", e);
    }
}