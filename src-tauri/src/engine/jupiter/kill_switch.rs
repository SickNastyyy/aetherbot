use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ── KillSwitch ───────────────────────────────────────────────────────────────

/// Atomic kill switch. One command blocks all new swap tickets immediately.
/// Safe to clone and share across threads.
#[derive(Clone)]
pub struct KillSwitch(Arc<AtomicBool>);

impl KillSwitch {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Activate the kill switch — blocks all new tickets instantly.
    pub fn activate(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Deactivate — allows new tickets again (requires owner intent).
    pub fn deactivate(&self) {
        self.0.store(false, Ordering::SeqCst);
    }

    pub fn is_active(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl Default for KillSwitch {
    fn default() -> Self {
        Self::new()
    }
}
