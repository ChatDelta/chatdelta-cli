//! Metrics display module for ChatDelta CLI
//! Uses the core library's ClientMetrics for consistent performance tracking

use chatdelta::{ClientMetrics, MetricsSnapshot};
use std::collections::HashMap;
use std::io::Write;
use chrono::{DateTime, Utc};

/// Metrics collector for the CLI session
pub struct CliMetrics {
    /// Per-provider metrics using the core library's ClientMetrics
    provider_metrics: HashMap<String, ClientMetrics>,
    /// Overall session metrics
    session_metrics: ClientMetrics,
    /// Session start time
    start_time: DateTime<Utc>,
}

impl CliMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            provider_metrics: HashMap::new(),
            session_metrics: ClientMetrics::new(),
            start_time: Utc::now(),
        }
    }
    
    /// Get or create metrics for a provider
    pub fn get_provider_metrics(&mut self, provider: &str) -> ClientMetrics {
        self.provider_metrics
            .entry(provider.to_string())
            .or_insert_with(ClientMetrics::new)
            .clone()
    }
    
    /// Record a successful API call
    pub fn record_success(&mut self, provider: &str, latency_ms: u64, tokens: Option<u32>) {
        if let Some(metrics) = self.provider_metrics.get(provider) {
            metrics.record_request(true, latency_ms, tokens);
        }
        self.session_metrics.record_request(true, latency_ms, tokens);
    }
    
    /// Record a failed API call
    pub fn record_failure(&mut self, provider: &str, latency_ms: u64) {
        if let Some(metrics) = self.provider_metrics.get(provider) {
            metrics.record_request(false, latency_ms, None);
        }
        self.session_metrics.record_request(false, latency_ms, None);
    }
    
    /// Get session summary
    pub fn get_session_summary(&self) -> SessionSummary {
        let session_stats = self.session_metrics.get_stats();
        let duration = Utc::now().signed_duration_since(self.start_time);
        
        let mut provider_summaries = Vec::new();
        for (name, metrics) in &self.provider_metrics {
            provider_summaries.push((name.clone(), metrics.get_stats()));
        }
        
        SessionSummary {
            duration_seconds: duration.num_seconds() as u64,
            total_requests: session_stats.requests_total,
            success_rate: session_stats.success_rate,
            average_latency_ms: session_stats.average_latency_ms,
            total_tokens: session_stats.total_tokens_used,
            provider_stats: provider_summaries,
        }
    }
    
    /// Display metrics in a formatted table
    pub fn display_metrics(&self, verbose: bool) {
        let summary = self.get_session_summary();
        
        println!("\n📊 Performance Metrics");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Session overview
        println!("📍 Session Duration: {}s", summary.duration_seconds);
        println!("📍 Total Requests: {}", summary.total_requests);
        println!("📍 Success Rate: {:.1}%", summary.success_rate);
        println!("📍 Avg Latency: {}ms", summary.average_latency_ms);
        
        if summary.total_tokens > 0 {
            println!("📍 Total Tokens: {}", summary.total_tokens);
        }
        
        if verbose && !summary.provider_stats.is_empty() {
            println!("\n🔍 Per-Provider Breakdown:");
            for (provider, stats) in &summary.provider_stats {
                println!("\n  {}:", provider);
                println!("    • Requests: {} (Success: {:.1}%)", 
                    stats.requests_total, stats.success_rate);
                println!("    • Avg Latency: {}ms", stats.average_latency_ms);
                if stats.total_tokens_used > 0 {
                    println!("    • Tokens Used: {}", stats.total_tokens_used);
                }
                if stats.cache_hit_rate > 0.0 {
                    println!("    • Cache Hit Rate: {:.1}%", stats.cache_hit_rate);
                }
            }
        }
        
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
    
    /// Export metrics as JSON
    pub fn export_json(&self) -> serde_json::Value {
        let summary = self.get_session_summary();
        serde_json::json!({
            "session": {
                "start_time": self.start_time.to_rfc3339(),
                "duration_seconds": summary.duration_seconds,
                "total_requests": summary.total_requests,
                "success_rate": summary.success_rate,
                "average_latency_ms": summary.average_latency_ms,
                "total_tokens": summary.total_tokens,
            },
            "providers": summary.provider_stats.iter().map(|(name, stats)| {
                (name.clone(), serde_json::json!({
                    "requests_total": stats.requests_total,
                    "requests_successful": stats.requests_successful,
                    "requests_failed": stats.requests_failed,
                    "success_rate": stats.success_rate,
                    "average_latency_ms": stats.average_latency_ms,
                    "total_tokens_used": stats.total_tokens_used,
                    "cache_hit_rate": stats.cache_hit_rate,
                }))
            }).collect::<serde_json::Map<_, _>>(),
        })
    }
    
    /// Save metrics to file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = self.export_json();
        let mut file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(&mut file, &json)?;
        file.flush()?;
        Ok(())
    }
}

/// Summary of session metrics
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub duration_seconds: u64,
    pub total_requests: u64,
    pub success_rate: f64,
    pub average_latency_ms: u64,
    pub total_tokens: u64,
    pub provider_stats: Vec<(String, MetricsSnapshot)>,
}

impl Default for CliMetrics {
    fn default() -> Self {
        Self::new()
    }
}