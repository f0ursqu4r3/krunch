//! Exponential jittered backoff (PLAN §3c). Kept pure — the caller (orchestrator)
//! supplies the jitter sample in `[0, 1)` so retry timing is deterministic in tests
//! while still jittered in production.

use std::time::Duration;

/// Delay before retry number `attempt` (0-based: attempt 0 is the first retry).
///
/// `base * 2^attempt`, capped at `max`, then "full jitter": the returned delay is
/// `jitter01 * capped` (a random point in `[0, capped)`), which decorrelates
/// concurrent seats' retries. `jitter01` must be in `[0, 1)`.
pub fn backoff_delay(attempt: u32, base: Duration, max: Duration, jitter01: f64) -> Duration {
    let jitter01 = jitter01.clamp(0.0, 1.0);
    let factor = 2u64.saturating_pow(attempt);
    let raw = base.saturating_mul(factor.min(u32::MAX as u64) as u32);
    let capped = raw.min(max);
    capped.mul_f64(jitter01)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_exponentially_before_the_cap() {
        let base = Duration::from_millis(100);
        let max = Duration::from_secs(30);
        // Full jitter at the top of the window reproduces the exponential schedule.
        assert_eq!(backoff_delay(0, base, max, 1.0), Duration::from_millis(100));
        assert_eq!(backoff_delay(1, base, max, 1.0), Duration::from_millis(200));
        assert_eq!(backoff_delay(2, base, max, 1.0), Duration::from_millis(400));
    }

    #[test]
    fn is_capped() {
        let base = Duration::from_secs(1);
        let max = Duration::from_secs(5);
        assert_eq!(backoff_delay(10, base, max, 1.0), Duration::from_secs(5));
    }

    #[test]
    fn jitter_scales_within_the_window() {
        let base = Duration::from_millis(100);
        let max = Duration::from_secs(30);
        assert_eq!(backoff_delay(1, base, max, 0.0), Duration::ZERO);
        assert_eq!(backoff_delay(1, base, max, 0.5), Duration::from_millis(100));
    }

    #[test]
    fn out_of_range_jitter_is_clamped() {
        let base = Duration::from_millis(100);
        let max = Duration::from_secs(30);
        assert_eq!(backoff_delay(0, base, max, 5.0), Duration::from_millis(100));
        assert_eq!(backoff_delay(0, base, max, -1.0), Duration::ZERO);
    }
}
