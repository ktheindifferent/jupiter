#!/bin/bash

# Set required environment variables
export ACCUWEATHERKEY="test_api_key"
export ZIP_CODE="12345"

# Set database environment variables (mock values for testing)
export HOMEBREW_PG_DBNAME="test_db"
export HOMEBREW_PG_USER="test_user"
export HOMEBREW_PG_PASS="test_pass"
export HOMEBREW_PG_ADDRESS="localhost:5432"

export COMBO_PG_DBNAME="test_db"
export COMBO_PG_USER="test_user"
export COMBO_PG_PASS="test_pass"
export COMBO_PG_ADDRESS="localhost:5432"

# Start the server in background
cargo run &
SERVER_PID=$!

# Give server time to start
sleep 3

# Check if server is running
if ps -p $SERVER_PID > /dev/null; then
    echo "✓ Server is running with PID $SERVER_PID"
    
    # Check CPU usage (should be minimal)
    CPU_USAGE=$(ps -p $SERVER_PID -o %cpu= | tr -d ' ')
    echo "  CPU Usage: ${CPU_USAGE}%"
    
    # Send SIGTERM to gracefully shutdown
    echo "✓ Sending SIGTERM for graceful shutdown..."
    kill -TERM $SERVER_PID
    
    # Wait for graceful shutdown
    sleep 3
    
    # Check if process has ended
    if ps -p $SERVER_PID > /dev/null; then
        echo "✗ Server did not shut down gracefully, force killing..."
        kill -9 $SERVER_PID
        exit 1
    else
        echo "✓ Server shut down gracefully"
    fi
else
    echo "✗ Server failed to start"
    exit 1
fi

echo "✓ All tests passed!"