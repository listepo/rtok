//! Note ranking with optional recency decay (plan T69.2).

use std::time::{SystemTime, UNIX_EPOCH};

/// Score for ordering recall/search. `half_life_days = 0` keeps id-desc only via caller.
pub fn score(uses: i32, last_used: Option<i64>, now: i64, half_life_days: u32) -> f64 {
    if half_life_days == 0 {
        return 0.0;
    }
    let uses = uses.max(0) as f64;
    let age_days = last_used
        .map(|ts| ((now - ts).max(0) as f64) / 86_400.0)
        .unwrap_or(365.0);
    (1.0 + uses).ln() * 0.5_f64.powf(age_days / f64::from(half_life_days))
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn age_days(last_used: Option<i64>, now: i64) -> u32 {
    last_used
        .map(|ts| ((now - ts).max(0) as u32) / 86_400)
        .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_heavily_used_beats_fresh_unused_when_decay_on() {
        let now = 1_700_000_000_i64;
        let old = score(10, Some(now - 60 * 86_400), now, 30);
        let fresh = score(0, None, now, 30);
        assert!(old > fresh);
    }

    #[test]
    fn zero_half_life_scores_are_flat() {
        assert_eq!(score(10, Some(1), 2, 0), 0.0);
        assert_eq!(score(0, None, 2, 0), 0.0);
    }
}
