use std::{
    path::PathBuf,
    sync::Mutex,
    time::Duration,
};

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use tauri::{AppHandle, Emitter};

type ModsDebouncer = Debouncer<notify::RecommendedWatcher, FileIdMap>;

struct WatchState {
    _debouncer: ModsDebouncer,
}

static WATCH: Mutex<Option<WatchState>> = Mutex::new(None);

pub fn sync_mods_watch(app: &AppHandle, mods_dir: Option<PathBuf>) {
    let mut guard = match WATCH.lock() {
        Ok(value) => value,
        Err(_) => return,
    };
    *guard = None;

    let Some(dir) = mods_dir.filter(|path| path.is_dir()) else {
        return;
    };

    let handle = app.clone();
    let Ok(mut debouncer) = new_debouncer(
        Duration::from_millis(800),
        None,
        move |result: DebounceEventResult| {
            if result.is_err() {
                return;
            }
            let _ = handle.emit("mods-folder-changed", ());
        },
    ) else {
        return;
    };

    if debouncer.watch(&dir, RecursiveMode::Recursive).is_err() {
        return;
    }

    *guard = Some(WatchState {
        _debouncer: debouncer,
    });
}
