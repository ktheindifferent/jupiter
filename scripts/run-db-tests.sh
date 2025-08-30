#!/bin/bash

# Test runner script for database tests
# This script handles environment setup and database initialization for tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Weather API Database Test Runner${NC}"
echo "=================================="

# Check if Docker is available
if command -v docker &> /dev/null && command -v docker-compose &> /dev/null; then
    echo -e "${GREEN}✓${NC} Docker and docker-compose found"
    USE_DOCKER=true
else
    echo -e "${YELLOW}⚠${NC} Docker not found. Tests will use environment variables or defaults."
    USE_DOCKER=false
fi

# Function to cleanup Docker containers
cleanup_docker() {
    if [ "$USE_DOCKER" = true ] && [ "$DOCKER_STARTED" = true ]; then
        echo -e "\n${YELLOW}Cleaning up test containers...${NC}"
        docker-compose -f docker-compose.test.yml down -v
    fi
}

# Set up trap to cleanup on exit
trap cleanup_docker EXIT

# Start test database if using Docker
DOCKER_STARTED=false
if [ "$USE_DOCKER" = true ]; then
    echo -e "\n${GREEN}Starting test database containers...${NC}"
    docker-compose -f docker-compose.test.yml up -d
    DOCKER_STARTED=true
    
    # Wait for database to be ready
    echo -e "${YELLOW}Waiting for database to be ready...${NC}"
    sleep 5
    
    # Set environment variables for Docker database
    export HOMEBREW_PG_DBNAME=test_homebrew_db
    export HOMEBREW_PG_USER=postgres
    export HOMEBREW_PG_PASS=password
    export HOMEBREW_PG_ADDRESS=localhost
    
    export COMBO_PG_DBNAME=test_combo_db
    export COMBO_PG_USER=postgres
    export COMBO_PG_PASS=password
    export COMBO_PG_ADDRESS=localhost
    
    echo -e "${GREEN}✓${NC} Test database ready"
fi

# Load environment variables from .env.test if it exists
if [ -f .env.test ]; then
    echo -e "${GREEN}Loading environment from .env.test${NC}"
    export $(cat .env.test | grep -v '^#' | xargs)
fi

# Run tests
echo -e "\n${GREEN}Running database tests...${NC}"
echo "=================================="

if [ "$1" = "--verbose" ]; then
    RUST_BACKTRACE=1 cargo test db_pool_tests -- --nocapture
else
    cargo test db_pool_tests
fi

TEST_RESULT=$?

if [ $TEST_RESULT -eq 0 ]; then
    echo -e "\n${GREEN}✓ All tests passed!${NC}"
else
    echo -e "\n${RED}✗ Some tests failed${NC}"
fi

exit $TEST_RESULT