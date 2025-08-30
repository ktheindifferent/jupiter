-- Test Database Initialization Script
-- This script sets up the test databases with necessary schemas and permissions

-- Create additional test database
CREATE DATABASE test_combo_db;

-- Grant permissions
GRANT ALL PRIVILEGES ON DATABASE test_homebrew_db TO postgres;
GRANT ALL PRIVILEGES ON DATABASE test_combo_db TO postgres;

-- Connect to test_homebrew_db and create test schema
\c test_homebrew_db;

CREATE SCHEMA IF NOT EXISTS test;
GRANT ALL ON SCHEMA test TO postgres;

-- Create a simple test table for connection verification
CREATE TABLE IF NOT EXISTS test.connection_check (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Connect to test_combo_db and create test schema
\c test_combo_db;

CREATE SCHEMA IF NOT EXISTS test;
GRANT ALL ON SCHEMA test TO postgres;

-- Create a simple test table for connection verification
CREATE TABLE IF NOT EXISTS test.connection_check (
    id SERIAL PRIMARY KEY,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);