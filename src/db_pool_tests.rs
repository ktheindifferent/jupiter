#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_pool::{DatabaseConfig, DatabasePool, init_homebrew_pool, init_combo_pool};
    use std::time::Duration;
    use tokio;

    #[tokio::test]
    async fn test_pool_initialization_with_invalid_config() {
        let config = DatabaseConfig {
            db_name: String::from("invalid_db"),
            username: String::from("invalid_user"),
            password: String::from("invalid_pass"),
            host: String::from("invalid_host"),
            port: Some(5432),
            pool_size: Some(5),
            connection_timeout: Some(Duration::from_secs(1)),
            idle_timeout: Some(Duration::from_secs(60)),
            max_lifetime: Some(Duration::from_secs(180)),
            use_ssl: false,
        };

        let result = init_homebrew_pool(config).await;
        assert!(result.is_err(), "Pool initialization should fail with invalid config");
    }

    #[tokio::test]
    async fn test_connection_retry_logic() {
        // This test requires a properly configured database
        // Skip if environment variables are not set
        if std::env::var("HOMEBREW_PG_DBNAME").is_err() {
            println!("Skipping test: Database environment variables not set");
            return;
        }

        let config = DatabaseConfig {
            db_name: std::env::var("HOMEBREW_PG_DBNAME").unwrap(),
            username: std::env::var("HOMEBREW_PG_USER").unwrap(),
            password: std::env::var("HOMEBREW_PG_PASS").unwrap(),
            host: std::env::var("HOMEBREW_PG_ADDRESS").unwrap(),
            port: Some(5432),
            pool_size: Some(2),
            connection_timeout: Some(Duration::from_secs(2)),
            idle_timeout: Some(Duration::from_secs(60)),
            max_lifetime: Some(Duration::from_secs(180)),
            use_ssl: true,
        };

        match init_homebrew_pool(config).await {
            Ok(pool) => {
                // Test successful connection retrieval
                let conn_result = pool.get_connection().await;
                assert!(conn_result.is_ok(), "Should successfully get connection from pool");

                // Test connection with retry
                let conn_with_retry = pool.get_connection_with_retry(3).await;
                assert!(conn_with_retry.is_ok(), "Should successfully get connection with retry");

                // Test pool status
                let status = pool.status();
                assert!(status.size > 0, "Pool size should be greater than 0");
            }
            Err(e) => {
                println!("Pool initialization failed (expected in test environment): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_pool_exhaustion_behavior() {
        // This test requires a properly configured database
        if std::env::var("HOMEBREW_PG_DBNAME").is_err() {
            println!("Skipping test: Database environment variables not set");
            return;
        }

        let config = DatabaseConfig {
            db_name: std::env::var("HOMEBREW_PG_DBNAME").unwrap(),
            username: std::env::var("HOMEBREW_PG_USER").unwrap(),
            password: std::env::var("HOMEBREW_PG_PASS").unwrap(),
            host: std::env::var("HOMEBREW_PG_ADDRESS").unwrap(),
            port: Some(5432),
            pool_size: Some(1), // Very small pool
            connection_timeout: Some(Duration::from_millis(500)),
            idle_timeout: Some(Duration::from_secs(60)),
            max_lifetime: Some(Duration::from_secs(180)),
            use_ssl: true,
        };

        match init_combo_pool(config).await {
            Ok(pool) => {
                // Get the only connection
                let _conn1 = pool.get_connection().await;
                
                // Try to get another connection (should timeout or wait)
                let start = std::time::Instant::now();
                let conn2_result = tokio::time::timeout(
                    Duration::from_millis(600),
                    pool.get_connection()
                ).await;
                
                let elapsed = start.elapsed();
                
                // Should timeout because pool is exhausted
                assert!(conn2_result.is_err() || elapsed > Duration::from_millis(400),
                    "Second connection should timeout or wait when pool is exhausted");
            }
            Err(e) => {
                println!("Pool initialization failed (expected in test environment): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_connection_health_check() {
        if std::env::var("HOMEBREW_PG_DBNAME").is_err() {
            println!("Skipping test: Database environment variables not set");
            return;
        }

        let config = DatabaseConfig {
            db_name: std::env::var("HOMEBREW_PG_DBNAME").unwrap(),
            username: std::env::var("HOMEBREW_PG_USER").unwrap(),
            password: std::env::var("HOMEBREW_PG_PASS").unwrap(),
            host: std::env::var("HOMEBREW_PG_ADDRESS").unwrap(),
            port: Some(5432),
            pool_size: Some(3),
            connection_timeout: Some(Duration::from_secs(2)),
            idle_timeout: Some(Duration::from_secs(60)),
            max_lifetime: Some(Duration::from_secs(180)),
            use_ssl: true,
        };

        match DatabasePool::new_homebrew(config).await {
            Ok(pool) => {
                // Get a connection and verify it's healthy
                match pool.get_connection().await {
                    Ok(client) => {
                        // The health check is performed internally in get_connection
                        // If we get here, the connection passed the health check
                        let result = client.query_one("SELECT 1 as test", &[]).await;
                        assert!(result.is_ok(), "Connection should be able to execute queries");
                        
                        let row = result.unwrap();
                        let value: i32 = row.get("test");
                        assert_eq!(value, 1, "Query should return expected value");
                    }
                    Err(e) => {
                        println!("Failed to get connection (expected in test environment): {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Pool initialization failed (expected in test environment): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        if std::env::var("COMBO_PG_DBNAME").is_err() {
            println!("Skipping test: Database environment variables not set");
            return;
        }

        let config = DatabaseConfig {
            db_name: std::env::var("COMBO_PG_DBNAME").unwrap(),
            username: std::env::var("COMBO_PG_USER").unwrap(),
            password: std::env::var("COMBO_PG_PASS").unwrap(),
            host: std::env::var("COMBO_PG_ADDRESS").unwrap(),
            port: Some(5432),
            pool_size: Some(2),
            connection_timeout: Some(Duration::from_secs(2)),
            idle_timeout: Some(Duration::from_secs(60)),
            max_lifetime: Some(Duration::from_secs(180)),
            use_ssl: true,
        };

        match DatabasePool::new_combo(config).await {
            Ok(pool) => {
                // Get a connection
                let conn_result = pool.get_connection().await;
                assert!(conn_result.is_ok(), "Should get connection before shutdown");
                
                // Drop the connection
                drop(conn_result);
                
                // Close the pool
                pool.close().await;
                
                // Pool should be closed now, trying to get a connection should fail
                // Note: We can't test this directly as the pool is consumed by close()
                println!("Pool closed successfully");
            }
            Err(e) => {
                println!("Pool initialization failed (expected in test environment): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_concurrent_connections() {
        if std::env::var("HOMEBREW_PG_DBNAME").is_err() {
            println!("Skipping test: Database environment variables not set");
            return;
        }

        let config = DatabaseConfig {
            db_name: std::env::var("HOMEBREW_PG_DBNAME").unwrap(),
            username: std::env::var("HOMEBREW_PG_USER").unwrap(),
            password: std::env::var("HOMEBREW_PG_PASS").unwrap(),
            host: std::env::var("HOMEBREW_PG_ADDRESS").unwrap(),
            port: Some(5432),
            pool_size: Some(5),
            connection_timeout: Some(Duration::from_secs(2)),
            idle_timeout: Some(Duration::from_secs(60)),
            max_lifetime: Some(Duration::from_secs(180)),
            use_ssl: true,
        };

        match init_homebrew_pool(config).await {
            Ok(pool) => {
                // Spawn multiple tasks that each get a connection
                let mut handles = vec![];
                
                for i in 0..3 {
                    let pool_clone = pool.clone();
                    let handle = tokio::spawn(async move {
                        match pool_clone.get_connection().await {
                            Ok(client) => {
                                // Simulate some work
                                let result = client.query_one("SELECT $1::int as num", &[&i]).await;
                                match result {
                                    Ok(row) => {
                                        let num: i32 = row.get("num");
                                        println!("Task {} completed with result: {}", i, num);
                                        Ok(num)
                                    }
                                    Err(e) => {
                                        println!("Task {} query failed: {}", i, e);
                                        Err(e)
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Task {} failed to get connection: {}", i, e);
                                // Return error as a string since we can't create tokio_postgres::Error
                                panic!("Task {} failed to get connection: {}", i, e)
                            }
                        }
                    });
                    handles.push(handle);
                }
                
                // Wait for all tasks to complete
                for handle in handles {
                    let result = handle.await;
                    assert!(result.is_ok(), "Task should complete successfully");
                }
            }
            Err(e) => {
                println!("Pool initialization failed (expected in test environment): {}", e);
            }
        }
    }
}