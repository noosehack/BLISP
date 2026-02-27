//! Simple Allocation Tracker
//!
//! Tracks Column allocations by counting new_f64 calls.
//! This gives us a precise count of intermediate allocations.

use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Reset the allocation counter
pub fn reset_allocations() {
    ALLOCATION_COUNT.store(0, Ordering::SeqCst);
}

/// Get the current allocation count
pub fn get_allocations() -> usize {
    ALLOCATION_COUNT.load(Ordering::SeqCst)
}

/// Increment allocation counter
pub fn track_allocation() {
    ALLOCATION_COUNT.fetch_add(1, Ordering::SeqCst);
}

/// Run a closure and return (result, allocation_count)
pub fn measure_allocations<F, R>(f: F) -> (R, usize)
where
    F: FnOnce() -> R,
{
    reset_allocations();
    let result = f();
    let count = get_allocations();
    (result, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation_tracking() {
        reset_allocations();
        assert_eq!(get_allocations(), 0);

        track_allocation();
        assert_eq!(get_allocations(), 1);

        track_allocation();
        track_allocation();
        assert_eq!(get_allocations(), 3);

        reset_allocations();
        assert_eq!(get_allocations(), 0);
    }

    #[test]
    fn test_measure_allocations() {
        let (result, count) = measure_allocations(|| {
            track_allocation();
            track_allocation();
            42
        });

        assert_eq!(result, 42);
        assert_eq!(count, 2);
    }
}
