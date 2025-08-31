use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH, Instant};
use std::fmt;

static LAST_KNOWN_TIMESTAMP: AtomicI64 = AtomicI64::new(0);

#[derive(Debug, Clone)]
pub enum TimeError {
    SystemTimeError(String),
    InvalidTimestamp(i64),
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeError::SystemTimeError(msg) => write!(f, "System time error: {}", msg),
            TimeError::InvalidTimestamp(ts) => write!(f, "Invalid timestamp: {}", ts),
        }
    }
}

impl std::error::Error for TimeError {}

impl From<SystemTimeError> for TimeError {
    fn from(err: SystemTimeError) -> Self {
        TimeError::SystemTimeError(err.to_string())
    }
}

pub fn safe_timestamp() -> Result<i64, TimeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| {
            let timestamp = d.as_secs() as i64;
            LAST_KNOWN_TIMESTAMP.store(timestamp, Ordering::Relaxed);
            timestamp
        })
        .map_err(|e| {
            log::warn!("System time error: {}", e);
            TimeError::from(e)
        })
}

pub fn safe_timestamp_with_fallback() -> i64 {
    safe_timestamp().unwrap_or_else(|e| {
        log::warn!("Using fallback timestamp due to: {}", e);
        let last_known = LAST_KNOWN_TIMESTAMP.load(Ordering::Relaxed);
        if last_known > 0 {
            last_known
        } else {
            log::error!("No valid timestamp available, using current epoch second estimate");
            1700000000
        }
    })
}

pub fn safe_timestamp_millis() -> Result<i64, TimeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| {
            let millis = d.as_millis() as i64;
            LAST_KNOWN_TIMESTAMP.store(d.as_secs() as i64, Ordering::Relaxed);
            millis
        })
        .map_err(|e| {
            log::warn!("System time error: {}", e);
            TimeError::from(e)
        })
}

pub fn validate_timestamp(timestamp: i64) -> Result<i64, TimeError> {
    const MIN_TIMESTAMP: i64 = 946684800; // Jan 1, 2000
    const MAX_TIMESTAMP: i64 = 2147483647; // Jan 19, 2038 (32-bit max)
    
    if timestamp < MIN_TIMESTAMP || timestamp > MAX_TIMESTAMP {
        Err(TimeError::InvalidTimestamp(timestamp))
    } else {
        Ok(timestamp)
    }
}

pub fn sanitize_timestamp(timestamp: i64) -> i64 {
    validate_timestamp(timestamp).unwrap_or_else(|_| {
        log::warn!("Timestamp {} out of bounds, using current time", timestamp);
        safe_timestamp_with_fallback()
    })
}

pub struct MonotonicTimer {
    start: Instant,
}

impl MonotonicTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
    
    pub fn elapsed_secs(&self) -> u64 {
        self.start.elapsed().as_secs()
    }
    
    pub fn elapsed_millis(&self) -> u128 {
        self.start.elapsed().as_millis()
    }
}

impl Default for MonotonicTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_safe_timestamp() {
        let result = safe_timestamp();
        assert!(result.is_ok());
        let timestamp = result.unwrap();
        assert!(timestamp > 0);
        assert!(timestamp < i64::MAX);
    }

    #[test]
    fn test_safe_timestamp_with_fallback() {
        let timestamp = safe_timestamp_with_fallback();
        assert!(timestamp > 0);
    }

    #[test]
    fn test_validate_timestamp() {
        assert!(validate_timestamp(1700000000).is_ok());
        
        assert!(validate_timestamp(0).is_err());
        
        assert!(validate_timestamp(i64::MAX).is_err());
        
        assert!(validate_timestamp(946684799).is_err());
    }

    #[test]
    fn test_sanitize_timestamp() {
        let valid = sanitize_timestamp(1700000000);
        assert_eq!(valid, 1700000000);
        
        let invalid = sanitize_timestamp(-1);
        assert!(invalid > 0);
    }

    #[test]
    fn test_monotonic_timer() {
        let timer = MonotonicTimer::new();
        thread::sleep(Duration::from_millis(100));
        
        let elapsed_millis = timer.elapsed_millis();
        assert!(elapsed_millis >= 100);
        assert!(elapsed_millis < 200);
    }

    #[test]
    fn test_last_known_timestamp_fallback() {
        let first = safe_timestamp().unwrap();
        LAST_KNOWN_TIMESTAMP.store(first, Ordering::Relaxed);
        
        let fallback = safe_timestamp_with_fallback();
        assert!(fallback > 0);
    }

    #[test]
    fn test_safe_timestamp_millis() {
        let result = safe_timestamp_millis();
        assert!(result.is_ok());
        let millis = result.unwrap();
        assert!(millis > 1000000000000); // After year 2001 in milliseconds
    }
}