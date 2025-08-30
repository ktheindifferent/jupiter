use crate::db_pool::{get_homebrew_pool, get_combo_pool};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;
use log::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMetrics {
    pub pool_name: String,
    pub size: usize,
    pub available: usize,
    pub waiting: usize,
    pub total_connections_created: u64,
    pub total_connections_recycled: u64,
    pub total_connection_errors: u64,
    pub average_wait_time_ms: u64,
    pub timestamp: i64,
}

pub struct PoolMonitor {
    total_connections_created: Arc<AtomicU64>,
    total_connections_recycled: Arc<AtomicU64>,
    total_connection_errors: Arc<AtomicU64>,
    total_wait_time_ms: Arc<AtomicU64>,
    wait_count: Arc<AtomicUsize>,
}

impl PoolMonitor {
    pub fn new() -> Self {
        Self {
            total_connections_created: Arc::new(AtomicU64::new(0)),
            total_connections_recycled: Arc::new(AtomicU64::new(0)),
            total_connection_errors: Arc::new(AtomicU64::new(0)),
            total_wait_time_ms: Arc::new(AtomicU64::new(0)),
            wait_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn record_connection_created(&self) {
        self.total_connections_created.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_connection_recycled(&self) {
        self.total_connections_recycled.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_connection_error(&self) {
        self.total_connection_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_wait_time(&self, wait_time_ms: u64) {
        self.total_wait_time_ms.fetch_add(wait_time_ms, Ordering::Relaxed);
        self.wait_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_average_wait_time(&self) -> u64 {
        let count = self.wait_count.load(Ordering::Relaxed);
        if count == 0 {
            0
        } else {
            self.total_wait_time_ms.load(Ordering::Relaxed) / count as u64
        }
    }

    pub fn get_metrics(&self, pool_name: String, size: usize, available: usize, waiting: usize) -> PoolMetrics {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or_else(|e| {
                log::error!("Failed to get system time: {}", e);
                0
            });
        
        PoolMetrics {
            pool_name,
            size,
            available,
            waiting,
            total_connections_created: self.total_connections_created.load(Ordering::Relaxed),
            total_connections_recycled: self.total_connections_recycled.load(Ordering::Relaxed),
            total_connection_errors: self.total_connection_errors.load(Ordering::Relaxed),
            average_wait_time_ms: self.get_average_wait_time(),
            timestamp,
        }
    }
}

// Global monitors
use tokio::sync::OnceCell;
use once_cell::sync::Lazy;

static HOMEBREW_MONITOR: Lazy<OnceCell<Arc<PoolMonitor>>> = Lazy::new(|| OnceCell::new());
static COMBO_MONITOR: Lazy<OnceCell<Arc<PoolMonitor>>> = Lazy::new(|| OnceCell::new());

pub async fn init_monitors() {
    let _ = HOMEBREW_MONITOR.get_or_init(|| async {
        Arc::new(PoolMonitor::new())
    }).await;
    
    let _ = COMBO_MONITOR.get_or_init(|| async {
        Arc::new(PoolMonitor::new())
    }).await;
    
    info!("Pool monitors initialized");
}

pub fn get_homebrew_monitor() -> Option<Arc<PoolMonitor>> {
    HOMEBREW_MONITOR.get().map(|m| Arc::clone(m))
}

pub fn get_combo_monitor() -> Option<Arc<PoolMonitor>> {
    COMBO_MONITOR.get().map(|m| Arc::clone(m))
}

pub fn get_all_pool_metrics() -> Vec<PoolMetrics> {
    let mut metrics = Vec::new();
    
    // Get homebrew pool metrics
    if let Some(pool) = get_homebrew_pool() {
        let status = pool.status();
        if let Some(monitor) = get_homebrew_monitor() {
            metrics.push(monitor.get_metrics(
                "homebrew".to_string(),
                status.size,
                status.available,
                status.waiting,
            ));
        }
    }
    
    // Get combo pool metrics
    if let Some(pool) = get_combo_pool() {
        let status = pool.status();
        if let Some(monitor) = get_combo_monitor() {
            metrics.push(monitor.get_metrics(
                "combo".to_string(),
                status.size,
                status.available,
                status.waiting,
            ));
        }
    }
    
    metrics
}

// Background monitoring task
pub async fn start_monitoring_task(interval_seconds: u64) {
    let interval = Duration::from_secs(interval_seconds);
    
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            
            let metrics = get_all_pool_metrics();
            for metric in metrics {
                if metric.available == 0 && metric.waiting > 0 {
                    warn!(
                        "[{}] Pool exhausted! Size: {}, Available: 0, Waiting: {}",
                        metric.pool_name, metric.size, metric.waiting
                    );
                } else if (metric.available as f64) / (metric.size as f64) < 0.2 {
                    warn!(
                        "[{}] Pool running low! Size: {}, Available: {}, Waiting: {}",
                        metric.pool_name, metric.size, metric.available, metric.waiting
                    );
                } else {
                    info!(
                        "[{}] Pool healthy - Size: {}, Available: {}, Waiting: {}",
                        metric.pool_name, metric.size, metric.available, metric.waiting
                    );
                }
                
                if metric.total_connection_errors > 0 {
                    error!(
                        "[{}] Connection errors detected: {}",
                        metric.pool_name, metric.total_connection_errors
                    );
                }
            }
        }
    });
}

// HTTP endpoint handler for metrics
pub fn handle_metrics_endpoint() -> String {
    let metrics = get_all_pool_metrics();
    serde_json::to_string_pretty(&metrics).unwrap_or_else(|e| {
        format!("{{\"error\": \"Failed to serialize metrics: {}\"}}", e)
    })
}