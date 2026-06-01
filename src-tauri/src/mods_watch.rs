use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use tauri::{AppHandle, Emitter};

type ModsDebouncer = Debouncer<notify::RecommendedWatcher, RecommendedCache>;

struct WatchState {
    _debouncer: ModsDebouncer,
}

static WATCH: Mutex<Option<WatchState>> = Mutex::new(None);
static SUPPRESS_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

pub fn suppress_events_for(duration: Duration) {
    let until = now_millis().saturating_add(duration.as_millis() as u64);
    let mut current = SUPPRESS_UNTIL_MS.load(Ordering::Relaxed);
    while until > current {
        match SUPPRESS_UNTIL_MS.compare_exchange(
            current,
            until,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

fn should_suppress_event() -> bool {
    now_millis() <= SUPPRESS_UNTIL_MS.load(Ordering::Relaxed)
}

pub fn sync_mods_watch(app: &AppHandle, mods_dirs: Vec<PathBuf>) {
    let mut guard = match WATCH.lock() {
        Ok(value) => value,
        Err(_) => return,
    };
    *guard = None;

    let dirs: Vec<PathBuf> = mods_dirs.into_iter().filter(|path| path.is_dir()).collect();
    if dirs.is_empty() {
        return;
    }

    let handle = app.clone();
    let Ok(mut debouncer) = new_debouncer(
        Duration::from_millis(800),
        None,
        move |result: DebounceEventResult| {
            if result.is_err() {
                return;
            }
            if should_suppress_event() {
                return;
            }
            let _ = handle.emit("mods-folder-changed", ());
        },
    ) else {
        return;
    };

    let mut watched = false;
    for dir in dirs {
        if debouncer.watch(&dir, RecursiveMode::Recursive).is_ok() {
            watched = true;
        }
    }
    if !watched {
        return;
    }

    *guard = Some(WatchState {
        _debouncer: debouncer,
    });
}
