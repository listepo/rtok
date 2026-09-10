//! Helpers shared by the integration tests (`mod common;` in each file that uses one).

use std::time::Duration;

/// Nearest-rank p95 of sorted samples: the smallest sample at or above 95 % of them. The old
/// `samples[n * 95 / 100]` was one rank high — at n = 20 it was the maximum, so one slow spawn on
/// a loaded CI runner failed a latency gate that 19 fast ones had passed.
pub fn p95(sorted: &[Duration]) -> Duration {
    sorted[(sorted.len() * 95).div_ceil(100) - 1]
}
