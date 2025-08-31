use jupiter::utils::time::{safe_timestamp, safe_timestamp_with_fallback, validate_timestamp, sanitize_timestamp, MonotonicTimer};

fn main() {
    println!("Testing safe_timestamp()...");
    match safe_timestamp() {
        Ok(ts) => println!("  ✓ Got timestamp: {}", ts),
        Err(e) => println!("  ✗ Error: {}", e),
    }
    
    println!("\nTesting safe_timestamp_with_fallback()...");
    let ts = safe_timestamp_with_fallback();
    println!("  ✓ Got fallback timestamp: {}", ts);
    
    println!("\nTesting validate_timestamp()...");
    let valid_ts = 1700000000;
    match validate_timestamp(valid_ts) {
        Ok(_) => println!("  ✓ Valid timestamp {} accepted", valid_ts),
        Err(e) => println!("  ✗ Error: {}", e),
    }
    
    let invalid_ts = -1;
    match validate_timestamp(invalid_ts) {
        Ok(_) => println!("  ✗ Invalid timestamp {} was incorrectly accepted", invalid_ts),
        Err(_) => println!("  ✓ Invalid timestamp {} correctly rejected", invalid_ts),
    }
    
    println!("\nTesting sanitize_timestamp()...");
    let sanitized = sanitize_timestamp(-1);
    println!("  ✓ Sanitized -1 to: {}", sanitized);
    
    println!("\nTesting MonotonicTimer...");
    let timer = MonotonicTimer::new();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let elapsed = timer.elapsed_millis();
    if elapsed >= 100 && elapsed < 200 {
        println!("  ✓ Timer correctly measured ~100ms: {}ms", elapsed);
    } else {
        println!("  ✗ Timer measurement unexpected: {}ms", elapsed);
    }
    
    println!("\nAll time utility tests completed!");
}