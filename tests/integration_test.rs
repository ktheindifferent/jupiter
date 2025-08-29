use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use std::env;

#[test]
fn test_server_starts_and_stops_gracefully() {
    // Set required environment variables
    env::set_var("ACCUWEATHERKEY", "test_key");
    env::set_var("ZIP_CODE", "12345");

    // Start the server in a subprocess
    let mut child = Command::new("cargo")
        .args(&["run"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start server: {}", e));

    // Give the server time to start
    thread::sleep(Duration::from_secs(3));

    // Check that the process is running
    match child.try_wait() {
        Ok(None) => {}, // Server is still running
        Ok(Some(status)) => panic!("Server exited unexpectedly with status: {:?}", status),
        Err(e) => panic!("Failed to check server status: {}", e),
    }

    // Send SIGTERM to the server
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;
        
        let pid = Pid::from_raw(child.id() as i32);
        if let Err(e) = signal::kill(pid, Signal::SIGTERM) {
            eprintln!("Failed to send SIGTERM: {}", e);
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = child.kill() {
            eprintln!("Failed to kill server: {}", e);
        }
    }

    // Wait for the server to shut down gracefully
    let result = child.wait_with_output()
        .unwrap_or_else(|e| panic!("Failed to wait for server shutdown: {}", e));
    
    // Check that the server exited successfully
    assert!(result.status.success() || result.status.code() == Some(0), 
            "Server should exit successfully");
}

#[test]
fn test_server_handles_ctrl_c_gracefully() {
    // Set required environment variables
    env::set_var("ACCUWEATHERKEY", "test_key");
    env::set_var("ZIP_CODE", "12345");

    // Start the server in a subprocess
    let mut child = Command::new("cargo")
        .args(&["run"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start server: {}", e));

    // Give the server time to start
    thread::sleep(Duration::from_secs(3));

    // Check that the process is running
    match child.try_wait() {
        Ok(None) => {}, // Server is still running
        Ok(Some(status)) => panic!("Server exited unexpectedly with status: {:?}", status),
        Err(e) => panic!("Failed to check server status: {}", e),
    }

    // Send SIGINT (Ctrl+C) to the server
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;
        
        let pid = Pid::from_raw(child.id() as i32);
        if let Err(e) = signal::kill(pid, Signal::SIGINT) {
            eprintln!("Failed to send SIGINT: {}", e);
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = child.kill() {
            eprintln!("Failed to kill server: {}", e);
        }
    }

    // Wait for the server to shut down gracefully
    let result = child.wait_with_output()
        .unwrap_or_else(|e| panic!("Failed to wait for server shutdown: {}", e));
    
    // Check that the server exited successfully
    assert!(result.status.success() || result.status.code() == Some(0), 
            "Server should exit successfully after Ctrl+C");
}

#[test]
fn test_server_fails_without_required_env_vars() {
    // Ensure environment variables are not set
    env::remove_var("ACCUWEATHERKEY");
    env::remove_var("ZIP_CODE");

    // Try to start the server
    let output = Command::new("cargo")
        .args(&["run"])
        .output()
        .unwrap_or_else(|e| panic!("Failed to execute server: {}", e));

    // Server should fail to start
    assert!(!output.status.success(), "Server should fail without required env vars");
    
    // Check error message contains expected text
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ACCUWEATHERKEY") || stderr.contains("ZIP_CODE"),
            "Error message should mention missing environment variables");
}