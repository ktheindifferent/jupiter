use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

#[tokio::test]
async fn test_async_mutex_basic() {
    // Test basic async mutex functionality
    let mutex = Arc::new(AsyncMutex::new(42));
    
    let mutex_clone = mutex.clone();
    let handle = tokio::spawn(async move {
        let mut guard = mutex_clone.lock().await;
        *guard = 100;
    });
    
    handle.await.unwrap();
    
    let value = *mutex.lock().await;
    assert_eq!(value, 100);
}

#[tokio::test]
async fn test_async_mutex_with_timeout() {
    // Test mutex with timeout pattern
    let mutex = Arc::new(AsyncMutex::new(Some("data".to_string())));
    
    // Test successful acquisition with timeout
    let result = tokio::time::timeout(Duration::from_secs(1), mutex.lock()).await;
    assert!(result.is_ok());
    
    let mut guard = result.unwrap();
    assert_eq!(*guard, Some("data".to_string()));
    *guard = None;
    drop(guard);
    
    // Verify value was changed
    let final_guard = mutex.lock().await;
    assert!(final_guard.is_none());
}

#[tokio::test]
async fn test_server_handle_pattern() {
    // Simulate the server handle pattern from homebrew/combo
    type ServerHandle = Arc<AsyncMutex<Option<JoinHandle<()>>>>;
    
    let server_handle: ServerHandle = Arc::new(AsyncMutex::new(None));
    
    // Simulate starting a server
    let handle = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(100));
        42
    });
    
    // Store handle in async mutex using timeout
    let handle_clone = server_handle.clone();
    tokio::spawn(async move {
        match tokio::time::timeout(Duration::from_secs(5), handle_clone.lock()).await {
            Ok(mut guard) => {
                *guard = Some(handle);
            },
            Err(_) => {
                panic!("Failed to acquire lock");
            }
        }
    }).await.unwrap();
    
    // Simulate shutdown with timeout
    let join_result = tokio::time::timeout(Duration::from_secs(10), async {
        match tokio::time::timeout(Duration::from_secs(2), server_handle.lock()).await {
            Ok(mut guard) => {
                if let Some(handle) = guard.take() {
                    // Join the thread
                    let result = tokio::task::spawn_blocking(move || {
                        handle.join().unwrap()
                    }).await.unwrap();
                    assert_eq!(result, 42);
                }
            },
            Err(_) => {
                panic!("Failed to acquire lock for shutdown");
            }
        }
    }).await;
    
    assert!(join_result.is_ok());
    
    // Verify handle is now None
    let final_guard = server_handle.lock().await;
    assert!(final_guard.is_none());
}