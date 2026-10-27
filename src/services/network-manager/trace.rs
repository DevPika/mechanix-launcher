//! Temporary, always-on Wi-Fi backend diagnostics.
use std::fmt;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Sequence numbers correlate service decisions and outgoing UI events.
/// Time is monotonic milliseconds since the first Wi-Fi trace in this process.
#[track_caller]
pub(crate) fn log(message: fmt::Arguments<'_>) {
    static START: OnceLock<Instant> = OnceLock::new();
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    let elapsed = START.get_or_init(Instant::now).elapsed();
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let caller = std::panic::Location::caller();
    eprintln!(
        "[wifi #{sequence:06} +{:.3}ms {}:{}] {message}",
        elapsed.as_secs_f64() * 1000.0,
        caller.file(),
        caller.line(),
    );
}

macro_rules! wifi_trace {
    ($($arg:tt)*) => {
        $crate::services::network_manager::trace::log(format_args!($($arg)*))
    };
}
pub(crate) use wifi_trace;
