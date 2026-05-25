use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{AppHandle, Manager};

pub struct BootstrapState {
    epoch: AtomicU64,
}

impl BootstrapState {
    pub fn new() -> Self {
        Self {
            epoch: AtomicU64::new(0),
        }
    }

    pub fn snapshot(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    pub fn cancel_active(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub fn is_active(&self, token: u64) -> bool {
        self.epoch.load(Ordering::Acquire) == token
    }
}

pub fn cancel_active_bootstrap(app: &AppHandle) {
    if let Some(state) = app.try_state::<BootstrapState>() {
        state.cancel_active();
    }
}

pub fn bootstrap_still_active(app: &AppHandle, token: u64) -> bool {
    app.try_state::<BootstrapState>()
        .map(|state| state.is_active(token))
        .unwrap_or(false)
}
