#!/bin/bash

# Test script to verify graceful shutdown with different signals

echo "Testing graceful shutdown with signals..."

# Set up environment variables for testing
export ACCUWEATHERKEY="test_key"
export ZIP_CODE="12345"
export HOMEBREW_PG_DBNAME="test_db"
export HOMEBREW_PG_USER="test_user"
export HOMEBREW_PG_PASS="test_pass"
export HOMEBREW_PG_ADDRESS="localhost:5432"
export COMBO_PG_DBNAME="test_db"
export COMBO_PG_USER="test_user"
export COMBO_PG_PASS="test_pass"
export COMBO_PG_ADDRESS="localhost:5432"

# Build the project
echo "Building the project..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

# Function to test a signal
test_signal() {
    local signal_name=$1
    local signal=$2
    
    echo ""
    echo "Testing $signal_name..."
    
    # Start the server in background
    ./target/release/jupiter &
    local pid=$!
    
    # Wait for server to start
    sleep 2
    
    # Send the signal
    echo "Sending $signal_name to PID $pid..."
    kill $signal $pid
    
    # Wait for the process to exit
    wait $pid
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo "$signal_name test PASSED - Server shut down gracefully"
    else
        echo "$signal_name test FAILED - Server did not shut down gracefully (exit code: $exit_code)"
    fi
}

# Test SIGTERM
test_signal "SIGTERM" "-TERM"

# Test SIGINT (Ctrl+C)
test_signal "SIGINT" "-INT"

# Test SIGHUP
test_signal "SIGHUP" "-HUP"

echo ""
echo "All signal tests completed!"