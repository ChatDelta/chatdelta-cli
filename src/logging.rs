//! Comprehensive logging functionality for ChatDelta CLI

use crate::cli::Args;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub interaction_id: String,
    pub prompt: String,
    pub responses: HashMap<String, ModelResponse>,
    pub summary: Option<String>,
    pub metrics: Option<PerformanceMetrics>,
    pub errors: Vec<ErrorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub model_name: String,
    pub response: String,
    pub response_time_ms: u128,
    pub tokens_used: Option<u32>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_time_ms: u128,
    pub parallel_execution_time_ms: u128,
    pub summary_generation_time_ms: Option<u128>,
    pub models_queried: u32,
    pub successful_responses: u32,
    pub failed_responses: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub error_type: String,
    pub message: String,
    pub retry_attempt: Option<u32>,
}

pub struct Logger {
    log_dir: PathBuf,
    session_id: String,
    log_format: String,
    enable_metrics: bool,
    enable_errors: bool,
    current_entry: Option<LogEntry>,
    start_time: Option<Instant>,
}

impl Logger {
    pub fn new(args: &Args) -> Result<Self, Box<dyn std::error::Error>> {
        let log_dir = args.log_dir.clone().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".chatdelta")
                .join("logs")
        });

        // Create log directory if it doesn't exist
        fs::create_dir_all(&log_dir)?;

        let session_id = args
            .session_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        Ok(Logger {
            log_dir,
            session_id,
            log_format: args.log_format.clone(),
            enable_metrics: args.log_metrics,
            enable_errors: args.log_errors,
            current_entry: None,
            start_time: None,
        })
    }

    pub fn start_interaction(&mut self, prompt: &str) {
        let interaction_id = Uuid::new_v4().to_string();
        self.start_time = Some(Instant::now());

        self.current_entry = Some(LogEntry {
            timestamp: Utc::now(),
            session_id: self.session_id.clone(),
            interaction_id,
            prompt: prompt.to_string(),
            responses: HashMap::new(),
            summary: None,
            metrics: None,
            errors: Vec::new(),
        });
    }

    pub fn log_model_response(
        &mut self,
        model_name: &str,
        response: Result<&str, &str>,
        response_time: Duration,
        tokens_used: Option<u32>,
    ) {
        if let Some(entry) = &mut self.current_entry {
            let model_response = ModelResponse {
                model_name: model_name.to_string(),
                response: match response {
                    Ok(resp) => resp.to_string(),
                    Err(_) => String::new(),
                },
                response_time_ms: response_time.as_millis(),
                tokens_used,
                success: response.is_ok(),
                error: response.err().map(|e| e.to_string()),
            };

            entry
                .responses
                .insert(model_name.to_string(), model_response);
        }
    }

    pub fn log_error(
        &mut self,
        model: &str,
        error_type: &str,
        message: &str,
        retry_attempt: Option<u32>,
    ) {
        if self.enable_errors {
            if let Some(entry) = &mut self.current_entry {
                entry.errors.push(ErrorEntry {
                    timestamp: Utc::now(),
                    model: model.to_string(),
                    error_type: error_type.to_string(),
                    message: message.to_string(),
                    retry_attempt,
                });
            }
        }
    }

    pub fn set_summary(&mut self, summary: &str) {
        if let Some(entry) = &mut self.current_entry {
            entry.summary = Some(summary.to_string());
        }
    }

    pub fn finalize_interaction(
        &mut self,
        summary_time: Option<Duration>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut entry) = self.current_entry.take() {
            if self.enable_metrics {
                let total_time = self.start_time.map(|t| t.elapsed()).unwrap_or_default();
                let successful = entry.responses.values().filter(|r| r.success).count() as u32;
                let failed = entry.responses.len() as u32 - successful;

                entry.metrics = Some(PerformanceMetrics {
                    total_time_ms: total_time.as_millis(),
                    parallel_execution_time_ms: entry
                        .responses
                        .values()
                        .map(|r| r.response_time_ms)
                        .max()
                        .unwrap_or(0),
                    summary_generation_time_ms: summary_time.map(|d| d.as_millis()),
                    models_queried: entry.responses.len() as u32,
                    successful_responses: successful,
                    failed_responses: failed,
                });
            }

            self.write_log_entry(&entry)?;
        }
        Ok(())
    }

    fn write_log_entry(&self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
        let filename = match self.log_format.as_str() {
            "json" => format!("{}.json", entry.timestamp.format("%Y%m%d")),
            "structured" => format!("{}.log", entry.timestamp.format("%Y%m%d")),
            _ => format!("{}.txt", entry.timestamp.format("%Y%m%d")),
        };

        let log_path = self.log_dir.join(filename);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        match self.log_format.as_str() {
            "json" => {
                writeln!(file, "{}", serde_json::to_string_pretty(entry)?)?;
            }
            "structured" => {
                writeln!(file, "=== INTERACTION {} ===", entry.interaction_id)?;
                writeln!(file, "Timestamp: {}", entry.timestamp)?;
                writeln!(file, "Session: {}", entry.session_id)?;
                writeln!(file, "Prompt: {}", entry.prompt)?;
                writeln!(file)?;

                for (model, response) in &entry.responses {
                    writeln!(file, "--- {} ---", model)?;
                    writeln!(file, "Success: {}", response.success)?;
                    writeln!(file, "Response Time: {}ms", response.response_time_ms)?;
                    if let Some(tokens) = response.tokens_used {
                        writeln!(file, "Tokens: {}", tokens)?;
                    }
                    if response.success {
                        writeln!(file, "Response: {}", response.response)?;
                    } else if let Some(error) = &response.error {
                        writeln!(file, "Error: {}", error)?;
                    }
                    writeln!(file)?;
                }

                if let Some(summary) = &entry.summary {
                    writeln!(file, "--- SUMMARY ---")?;
                    writeln!(file, "{}", summary)?;
                    writeln!(file)?;
                }

                if let Some(metrics) = &entry.metrics {
                    writeln!(file, "--- METRICS ---")?;
                    writeln!(file, "Total Time: {}ms", metrics.total_time_ms)?;
                    writeln!(
                        file,
                        "Parallel Execution: {}ms",
                        metrics.parallel_execution_time_ms
                    )?;
                    if let Some(summary_time) = metrics.summary_generation_time_ms {
                        writeln!(file, "Summary Generation: {}ms", summary_time)?;
                    }
                    writeln!(file, "Models Queried: {}", metrics.models_queried)?;
                    writeln!(file, "Successful: {}", metrics.successful_responses)?;
                    writeln!(file, "Failed: {}", metrics.failed_responses)?;
                    writeln!(file)?;
                }

                if !entry.errors.is_empty() {
                    writeln!(file, "--- ERRORS ---")?;
                    for error in &entry.errors {
                        writeln!(
                            file,
                            "[{}] {}: {} - {}",
                            error.timestamp, error.model, error.error_type, error.message
                        )?;
                    }
                    writeln!(file)?;
                }
                writeln!(file, "========================================")?;
                writeln!(file)?;
            }
            _ => {
                // simple format
                writeln!(
                    file,
                    "[{}] Session: {} | Interaction: {}",
                    entry.timestamp, entry.session_id, entry.interaction_id
                )?;
                writeln!(file, "Prompt: {}", entry.prompt)?;
                for (model, response) in &entry.responses {
                    writeln!(
                        file,
                        "{}: {}",
                        model,
                        if response.success {
                            "SUCCESS"
                        } else {
                            "FAILED"
                        }
                    )?;
                }
                if let Some(summary) = &entry.summary {
                    writeln!(file, "Summary: {}", summary)?;
                }
                writeln!(file, "---")?;
            }
        }

        Ok(())
    }

    pub fn get_log_stats(&self) -> Result<LogStats, Box<dyn std::error::Error>> {
        let mut stats = LogStats::default();

        for entry in fs::read_dir(&self.log_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                stats.total_files += 1;
                stats.total_size_bytes += entry.metadata()?.len();
            }
        }

        Ok(stats)
    }
}

#[derive(Debug, Default)]
pub struct LogStats {
    pub total_files: u32,
    pub total_size_bytes: u64,
    pub oldest_log: Option<DateTime<Utc>>,
    pub newest_log: Option<DateTime<Utc>>,
}

impl LogStats {
    pub fn size_human_readable(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = self.total_size_bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_index])
    }
}
