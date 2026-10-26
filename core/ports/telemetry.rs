//! Declares telemetry persistence contracts without coupling applications to SQLite.

use crate::domain::{
    errors::StorageError,
    models::{AggregatedStats, DeviceLoadStats, TaskMetrics},
};

/// Persists privacy-safe AI performance metrics and device load observations.
pub trait MetricsRepository {
    /// Stores one completed AI task metric.
    fn save_metrics(&self, metrics: TaskMetrics) -> Result<(), StorageError>;
    /// Loads completed metrics produced by one device.
    fn get_metrics_for_device(&self, device_id: &str) -> Result<Vec<TaskMetrics>, StorageError>;
    /// Loads every completed privacy-safe metric for dashboard aggregation.
    ///
    /// Inputs: None.
    /// Outputs: Metrics ordered by completion timestamp or a storage error.
    /// Side effects: Reads metadata only; task inputs, outputs, and audio remain unavailable.
    fn get_all_metrics(&self) -> Result<Vec<TaskMetrics>, StorageError>;
    /// Computes total task count, total duration, and average latency across stored metrics.
    fn get_aggregated_stats(&self) -> Result<AggregatedStats, StorageError>;
    /// Stores an instantaneous device load observation.
    fn save_device_load(&self, device_load_stats: DeviceLoadStats) -> Result<(), StorageError>;
}
