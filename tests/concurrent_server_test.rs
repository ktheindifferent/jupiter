use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinSet;

#[tokio::test]
async fn test_concurrent_mutex_access_no_deadlock() {
    // Simulate the server handle mutex pattern
    let mutex = Arc::new(AsyncMutex::new(Some(42)));
    let success_count = Arc::new(AtomicUsize::new(0));
    let timeout_count = Arc::new(AtomicUsize::new(0));
    
    let mut join_set = JoinSet::new();
    
    // Spawn 1000 concurrent tasks trying to access the mutex
    for i in 0..1000 {
        let mutex_clone = mutex.clone();
        let success_count_clone = success_count.clone();
        let timeout_count_clone = timeout_count.clone();
        
        join_set.spawn(async move {
            // Try to acquire lock with timeout
            match tokio::time::timeout(Duration::from_secs(5), mutex_clone.lock()).await {
                Ok(mut guard) => {
                    // Simulate some work
                    if i % 2 == 0 {
                        *guard = Some(i as i32);
                    } else {
                        let _ = guard.take();
                    }
                    tokio::time::sleep(Duration::from_micros(10)).await;
                    success_count_clone.fetch_add(1, Ordering::Relaxed);
                },
                Err(_) => {
                    timeout_count_clone.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    }
    
    // Wait for all tasks to complete
    while let Some(result) = join_set.join_next().await {
        result.expect("Task should not panic");
    }
    
    let successes = success_count.load(Ordering::Relaxed);
    let timeouts = timeout_count.load(Ordering::Relaxed);
    
    println!("Successful acquisitions: {}", successes);
    println!("Timed out acquisitions: {}", timeouts);
    
    // All tasks should succeed (no timeouts in normal operation)
    assert_eq!(successes, 1000);
    assert_eq!(timeouts, 0);
}

#[tokio::test]
async fn test_concurrent_read_write_pattern() {
    // Test the pattern of concurrent reads and writes
    let mutex = Arc::new(AsyncMutex::new(vec![1, 2, 3]));
    let mut join_set = JoinSet::new();
    
    // Spawn readers
    for _ in 0..500 {
        let mutex_clone = mutex.clone();
        join_set.spawn(async move {
            match tokio::time::timeout(Duration::from_secs(2), mutex_clone.lock()).await {
                Ok(guard) => {
                    // Read operation
                    let _sum: i32 = guard.iter().sum();
                    tokio::time::sleep(Duration::from_micros(10)).await;
                },
                Err(_) => {
                    panic!("Reader should not timeout");
                }
            }
        });
    }
    
    // Spawn writers
    for i in 0..500 {
        let mutex_clone = mutex.clone();
        join_set.spawn(async move {
            match tokio::time::timeout(Duration::from_secs(2), mutex_clone.lock()).await {
                Ok(mut guard) => {
                    // Write operation
                    guard.push(i);
                    tokio::time::sleep(Duration::from_micros(10)).await;
                },
                Err(_) => {
                    panic!("Writer should not timeout");
                }
            }
        });
    }
    
    // Wait for all tasks to complete
    while let Some(result) = join_set.join_next().await {
        result.expect("Task should not panic");
    }
    
    // Verify final state
    let final_guard = mutex.lock().await;
    assert_eq!(final_guard.len(), 503); // Original 3 + 500 writes
}

#[tokio::test]
async fn test_shutdown_pattern_no_deadlock() {
    // Simulate the shutdown pattern with server handle
    let server_handle = Arc::new(AsyncMutex::new(Some(42)));
    let shutdown_count = Arc::new(AtomicUsize::new(0));
    
    let mut join_set = JoinSet::new();
    
    // Spawn multiple tasks trying to shutdown concurrently
    for _ in 0..100 {
        let handle_clone = server_handle.clone();
        let shutdown_count_clone = shutdown_count.clone();
        
        join_set.spawn(async move {
            // Simulate shutdown with timeout pattern
            let timeout = Duration::from_secs(10);
            let join_result = tokio::time::timeout(timeout, async {
                match tokio::time::timeout(Duration::from_secs(2), handle_clone.lock()).await {
                    Ok(mut guard) => {
                        if let Some(_handle) = guard.take() {
                            // Simulate joining thread
                            tokio::time::sleep(Duration::from_millis(1)).await;
                            shutdown_count_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    },
                    Err(_) => {
                        // Log but don't panic
                        eprintln!("Failed to acquire lock for shutdown");
                    }
                }
            }).await;
            
            if join_result.is_err() {
                // Timeout - try force cleanup
                if let Ok(mut guard) = tokio::time::timeout(
                    Duration::from_secs(1),
                    handle_clone.lock()
                ).await {
                    guard.take();
                }
            }
        });
    }
    
    // Wait for all tasks to complete
    while let Some(result) = join_set.join_next().await {
        result.expect("Task should not panic");
    }
    
    // Only the first task should successfully shutdown
    let shutdowns = shutdown_count.load(Ordering::Relaxed);
    assert_eq!(shutdowns, 1, "Only one task should successfully take the handle");
    
    // Verify handle is now None
    let final_guard = server_handle.lock().await;
    assert!(final_guard.is_none(), "Handle should be None after shutdown");
}

#[tokio::test]
async fn test_high_contention_no_starvation() {
    // Test that under high contention, all tasks eventually get access
    let mutex = Arc::new(AsyncMutex::new(0u64));
    let completed = Arc::new(AtomicUsize::new(0));
    
    let mut join_set = JoinSet::new();
    
    // Create high contention scenario
    for i in 0..200 {
        let mutex_clone = mutex.clone();
        let completed_clone = completed.clone();
        
        join_set.spawn(async move {
            // Each task tries to increment the counter multiple times
            for _ in 0..10 {
                match tokio::time::timeout(Duration::from_secs(30), mutex_clone.lock()).await {
                    Ok(mut guard) => {
                        *guard += 1;
                        // Simulate some work to increase contention
                        if i % 10 == 0 {
                            tokio::time::sleep(Duration::from_micros(100)).await;
                        }
                    },
                    Err(_) => {
                        panic!("Task {} timed out - possible starvation", i);
                    }
                }
            }
            completed_clone.fetch_add(1, Ordering::Relaxed);
        });
    }
    
    // Wait for all tasks with a timeout
    let timeout = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(result) = join_set.join_next().await {
            result.expect("Task should not panic");
        }
    }).await;
    
    assert!(timeout.is_ok(), "All tasks should complete within timeout");
    
    let total_completed = completed.load(Ordering::Relaxed);
    assert_eq!(total_completed, 200, "All tasks should complete");
    
    let final_value = *mutex.lock().await;
    assert_eq!(final_value, 2000, "Counter should be incremented 2000 times");
}

#[tokio::test]
async fn test_nested_timeout_pattern() {
    // Test the nested timeout pattern used in shutdown
    let mutex = Arc::new(AsyncMutex::new(Some("data".to_string())));
    
    // Outer timeout
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        // Inner timeout for lock acquisition
        match tokio::time::timeout(Duration::from_secs(2), mutex.lock()).await {
            Ok(mut guard) => {
                // Simulate some async work
                tokio::time::sleep(Duration::from_millis(100)).await;
                guard.take()
            },
            Err(_) => {
                None
            }
        }
    }).await;
    
    assert!(result.is_ok(), "Nested timeout should complete successfully");
    assert_eq!(result.unwrap(), Some("data".to_string()));
    
    // Verify mutex is accessible and empty
    let guard = mutex.lock().await;
    assert!(guard.is_none());
}