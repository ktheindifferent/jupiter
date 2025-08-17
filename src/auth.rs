use rouille::{Request, Response};
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Performs constant-time comparison of two byte slices to prevent timing attacks
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    
    result == 0
}

/// Rate limiter for authentication attempts
pub struct RateLimiter {
    attempts: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_attempts: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_attempts: usize, window_seconds: u64) -> Self {
        RateLimiter {
            attempts: Arc::new(Mutex::new(HashMap::new())),
            max_attempts,
            window: Duration::from_secs(window_seconds),
        }
    }

    pub fn check_rate_limit(&self, client_id: &str) -> bool {
        let mut attempts = self.attempts.lock().unwrap();
        let now = Instant::now();
        
        // Get or create the attempt list for this client
        let client_attempts = attempts.entry(client_id.to_string()).or_insert_with(Vec::new);
        
        // Remove old attempts outside the window
        client_attempts.retain(|&attempt| now.duration_since(attempt) < self.window);
        
        // Check if we're under the limit
        if client_attempts.len() < self.max_attempts {
            client_attempts.push(now);
            true
        } else {
            false
        }
    }
}

/// Validates the authorization header and performs authentication
pub fn validate_auth_header(
    request: &Request,
    api_key: &str,
    rate_limiter: Option<&RateLimiter>,
) -> Result<(), Response> {
    // Get client identifier (IP address)
    let client_id = request.remote_addr().to_string();
    
    // Check rate limit if enabled
    if let Some(limiter) = rate_limiter {
        if !limiter.check_rate_limit(&client_id) {
            log::warn!("Rate limit exceeded for client: {}", client_id);
            return Err(Response::text("Too Many Requests")
                .with_status_code(429)
                .with_additional_header("Retry-After", "60"));
        }
    }
    
    // Get the Authorization header
    let auth_header = request.header("Authorization");
    
    match auth_header {
        Some(header_value) => {
            // Use constant-time comparison to prevent timing attacks
            if !constant_time_eq(header_value.as_bytes(), api_key.as_bytes()) {
                log::warn!("Authentication failed from IP: {}", client_id);
                return Err(Response::text("Unauthorized")
                    .with_status_code(401)
                    .with_additional_header("WWW-Authenticate", "Bearer"));
            }
            Ok(())
        }
        None => {
            log::warn!("Missing Authorization header from IP: {}", client_id);
            Err(Response::text("Unauthorized")
                .with_status_code(401)
                .with_additional_header("WWW-Authenticate", "Bearer"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hello!"));
        assert!(!constant_time_eq(b"", b"hello"));
    }

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(3, 60);
        
        // First 3 attempts should succeed
        assert!(limiter.check_rate_limit("client1"));
        assert!(limiter.check_rate_limit("client1"));
        assert!(limiter.check_rate_limit("client1"));
        
        // 4th attempt should fail
        assert!(!limiter.check_rate_limit("client1"));
        
        // Different client should succeed
        assert!(limiter.check_rate_limit("client2"));
    }
}