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
        .expect("Failed to start server");

    // Give the server time to start
    thread::sleep(Duration::from_secs(3));

    // Check that the process is running
    assert!(child.try_wait().unwrap().is_none(), "Server should still be running");

    // Send SIGTERM to the server
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;
        
        let pid = Pid::from_raw(child.id() as i32);
        signal::kill(pid, Signal::SIGTERM).expect("Failed to send SIGTERM");
    }

    #[cfg(not(unix))]
    {
        child.kill().expect("Failed to kill server");
    }

    // Wait for the server to shut down gracefully
    let result = child.wait_with_output().expect("Failed to wait for server shutdown");
    
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
        .expect("Failed to start server");

    // Give the server time to start
    thread::sleep(Duration::from_secs(3));

    // Check that the process is running
    assert!(child.try_wait().unwrap().is_none(), "Server should still be running");

    // Send SIGINT (Ctrl+C) to the server
    #[cfg(unix)]
    {
        use nix::sys::signal::{self, Signal};
        use nix::unistd::Pid;
        
        let pid = Pid::from_raw(child.id() as i32);
        signal::kill(pid, Signal::SIGINT).expect("Failed to send SIGINT");
    }

    #[cfg(not(unix))]
    {
        child.kill().expect("Failed to kill server");
    }

    // Wait for the server to shut down gracefully
    let result = child.wait_with_output().expect("Failed to wait for server shutdown");
    
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
        .expect("Failed to execute server");

    // Server should fail to start
    assert!(!output.status.success(), "Server should fail without required env vars");
    
    // Check error message contains expected text
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ACCUWEATHERKEY") || stderr.contains("ZIP_CODE"),
            "Error message should mention missing environment variables");
}