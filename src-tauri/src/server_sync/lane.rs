use std::{
    collections::HashSet,
    fs,
    path::Path,
};

use tauri::AppHandle;

use crate::{
    catalog,
    mod_names::{normalized_match_key, strip_filename_decorations, strip_version_suffixes},
    mods::{scan_mods_for_settings, side_runs_on_server},
    settings::{read_settings, resolve_paths, ServerSyncSettings},
};

use super::{
    config::{clean_remote_dir, join_remote_path, sync_config, sync_config_error},
    remote::{
        count_remote_orphans, index_remote_dir, prune_remote_orphans, remote_file_matches,
        remote_orphan_names, upload_remote_file, RemoteDirIndex,
    },
    emit_server_sync_progress, ServerSyncBulkResult, ServerSyncDeleteItem, ServerSyncLanePreview,
    ServerSyncState, ServerSyncUpdatePair, SyncLane,
};

pub(super) fn remote_dir_for_lane(config: &ServerSyncSettings, lane: SyncLane) -> Option<String> {
    match lane {
        SyncLane::Server => clean_remote_dir(&config.server_mods_path),
        SyncLane::Distribution => clean_remote_dir(&config.distribution_mods_path),
    }
}

fn lane_config_error(lane: SyncLane, config: &ServerSyncSettings) -> Option<String> {
    if remote_dir_for_lane(config, lane).is_some() {
        return None;
    }
    Some(match lane {
        SyncLane::Server => "Укажите путь.".to_string(),
        SyncLane::Distribution => "Укажите путь.".to_string(),
    })
}

pub(super) fn mod_applies_to_lane(lane: SyncLane, side: &str) -> bool {
    match lane {
        SyncLane::Server => side_runs_on_server(side),
        SyncLane::Distribution => true,
    }
}

fn mod_needs_upload_for_lane(
    lane: SyncLane,
    config: &ServerSyncSettings,
    side: &str,
    filename: &str,
    local_size: u64,
    index: Option<&RemoteDirIndex>,
) -> bool {
    if !mod_applies_to_lane(lane, side) {
        return false;
    }
    if remote_dir_for_lane(config, lane).is_none() {
        return false;
    }
    !index
        .map(|value| remote_file_matches(value, filename, local_size))
        .unwrap_or(false)
}

fn upload_mod_for_lane(
    lane: SyncLane,
    config: &ServerSyncSettings,
    local_path: &Path,
    filename: &str,
    side: &str,
    index: Option<&RemoteDirIndex>,
) -> Result<bool, String> {
    if !mod_applies_to_lane(lane, side) {
        return Ok(false);
    }
    let Some(dir) = remote_dir_for_lane(config, lane) else {
        return Ok(false);
    };
    let local_size = fs::metadata(local_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let needs_upload = index
        .map(|value| !remote_file_matches(value, filename, local_size))
        .unwrap_or(true);
    if !needs_upload {
        return Ok(false);
    }
    let remote = join_remote_path(&dir, filename);
    upload_remote_file(&config.ssh_host, local_path, &remote)?;
    Ok(true)
}

fn lane_allowed_names(mods: &[crate::mods::ModEntry], lane: SyncLane) -> HashSet<String> {
    mods
        .iter()
        .filter(|entry| !entry.disabled)
        .filter(|entry| mod_applies_to_lane(lane, &entry.side))
        .map(|entry| entry.filename.clone())
        .collect()
}

pub(super) fn mod_sync_identity_key(filename: &str) -> String {
    let stem = filename.trim_end_matches(".jar");
    let clean = strip_filename_decorations(stem);
    normalized_match_key(&strip_version_suffixes(&clean))
}

#[derive(Clone, Debug)]
pub(super) struct SyncChangeDetails {
    pub to_update: u32,
    pub to_upload: u32,
    pub to_delete: u32,
    pub upload_names: Vec<String>,
    pub update_pairs: Vec<ServerSyncUpdatePair>,
    pub delete_names: Vec<String>,
}

struct LaneSyncAnalysis {
    config: ServerSyncSettings,
    mods: Vec<crate::mods::ModEntry>,
    pending: Vec<(crate::mods::ModEntry, std::path::PathBuf)>,
    already_synced: u32,
    already_synced_names: Vec<String>,
    total_all: u32,
    remote_count: u32,
    to_delete: u32,
    remote_index: RemoteDirIndex,
}

fn mod_entry_for_sync_filename<'a>(
    mods: &'a [crate::mods::ModEntry],
    filename: &str,
) -> Option<&'a crate::mods::ModEntry> {
    if let Some(exact) = mods.iter().find(|entry| entry.filename == filename) {
        return Some(exact);
    }
    let key = mod_sync_identity_key(filename);
    mods
        .iter()
        .find(|entry| mod_sync_identity_key(&entry.filename) == key)
}

fn delete_items_for_names(mods: &[crate::mods::ModEntry], names: &[String]) -> Vec<ServerSyncDeleteItem> {
    names
        .iter()
        .map(|name| {
            if let Some(entry) = mod_entry_for_sync_filename(mods, name) {
                ServerSyncDeleteItem {
                    filename: name.clone(),
                    side: entry.side.clone(),
                    library: entry.library,
                    technical: entry.technical,
                }
            } else {
                ServerSyncDeleteItem {
                    filename: name.clone(),
                    side: String::new(),
                    library: false,
                    technical: false,
                }
            }
        })
        .collect()
}

pub(super) fn classify_sync_changes(
    pending: &[String],
    orphans: &[String],
    local_lane_names: &[String],
) -> SyncChangeDetails {
    let mut pending = pending.to_vec();
    let mut update_pairs = Vec::new();
    let mut delete_names = Vec::new();

    for orphan in orphans {
        let key = mod_sync_identity_key(orphan);
        if let Some(pos) = pending.iter().position(|name| mod_sync_identity_key(name) == key) {
            let local = pending.remove(pos);
            update_pairs.push(ServerSyncUpdatePair {
                remote: orphan.clone(),
                local,
            });
            continue;
        }
        if let Some(local) = local_lane_names
            .iter()
            .find(|name| mod_sync_identity_key(name.as_str()) == key)
        {
            update_pairs.push(ServerSyncUpdatePair {
                remote: orphan.clone(),
                local: local.clone(),
            });
            continue;
        }
        delete_names.push(orphan.clone());
    }

    SyncChangeDetails {
        to_update: update_pairs.len() as u32,
        to_upload: pending.len() as u32,
        to_delete: delete_names.len() as u32,
        upload_names: pending,
        update_pairs,
        delete_names,
    }
}

fn classify_lane_orphans(
    mods: &[crate::mods::ModEntry],
    lane: SyncLane,
    pending_names: &[String],
    orphan_names: &[String],
) -> SyncChangeDetails {
    let local_lane_names: Vec<String> = lane_allowed_names(mods, lane).into_iter().collect();
    classify_sync_changes(pending_names, orphan_names, &local_lane_names)
}

fn prune_lane(
    config: &ServerSyncSettings,
    lane: SyncLane,
    mods: &[crate::mods::ModEntry],
    pending_names: &[String],
) -> Result<(usize, SyncChangeDetails), String> {
    if !config.delete_extra_remote_jars {
        return Ok((
            0,
            SyncChangeDetails {
                to_update: 0,
                to_upload: 0,
                to_delete: 0,
                upload_names: Vec::new(),
                update_pairs: Vec::new(),
                delete_names: Vec::new(),
            },
        ));
    }
    let Some(dir) = remote_dir_for_lane(config, lane) else {
        return Ok((
            0,
            SyncChangeDetails {
                to_update: 0,
                to_upload: 0,
                to_delete: 0,
                upload_names: Vec::new(),
                update_pairs: Vec::new(),
                delete_names: Vec::new(),
            },
        ));
    };
    let allowed = lane_allowed_names(mods, lane);
    let orphan_names = remote_orphan_names(&config.ssh_host, &dir, &allowed)?;
    let changes = classify_lane_orphans(mods, lane, pending_names, &orphan_names);
    let deleted = prune_remote_orphans(&config.ssh_host, &dir, &allowed)?;
    Ok((deleted, changes))
}

fn prepare_lane_sync(
    app: &AppHandle,
    lane: SyncLane,
) -> Result<(ServerSyncSettings, Vec<crate::mods::ModEntry>, crate::settings::InstancePaths), String> {
    let settings = read_settings(app)?;
    let Some(config) = sync_config(&settings) else {
        return Err(sync_config_error(&settings));
    };
    if let Some(message) = lane_config_error(lane, &config) {
        return Err(message);
    }
    if crate::ssh_util::ssh_config_hostname(&config.ssh_host).is_none() {
        return Err(format!("«{}» не в ~/.ssh/config.", config.ssh_host));
    }
    let paths = resolve_paths(&settings)?;
    let catalog_root = catalog::catalog_root(app).ok();
    let mods = scan_mods_for_settings(&settings, catalog_root)?;
    Ok((config, mods, paths))
}

fn analyze_lane_sync(app: &AppHandle, lane: SyncLane) -> Result<LaneSyncAnalysis, String> {
    let (config, mods, paths) = prepare_lane_sync(app, lane)?;

    let jobs: Vec<_> = mods
        .iter()
        .filter(|entry| !entry.disabled)
        .filter(|entry| mod_applies_to_lane(lane, &entry.side))
        .filter_map(|entry| {
            paths
                .resolve_mod_jar(&entry.filename)
                .map(|local_path| (entry.clone(), local_path))
        })
        .collect();
    let total_all = jobs.len() as u32;

    let remote_dir = remote_dir_for_lane(&config, lane).expect("lane path checked");
    let remote_index = index_remote_dir(&config.ssh_host, &remote_dir)?;
    let remote_count = remote_index.files.len() as u32;

    let mut pending = Vec::new();
    let mut already_synced = 0u32;
    let mut already_synced_names = Vec::new();
    for (entry, local_path) in jobs {
        let local_size = fs::metadata(&local_path).map(|metadata| metadata.len()).unwrap_or(0);
        if mod_needs_upload_for_lane(
            lane,
            &config,
            &entry.side,
            &entry.filename,
            local_size,
            Some(&remote_index),
        ) {
            pending.push((entry, local_path));
        } else {
            already_synced += 1;
            already_synced_names.push(entry.filename.clone());
        }
    }

    let to_delete = if config.delete_extra_remote_jars {
        let allowed: HashSet<String> = mods
            .iter()
            .filter(|entry| !entry.disabled)
            .filter(|entry| mod_applies_to_lane(lane, &entry.side))
            .map(|entry| entry.filename.clone())
            .collect();
        count_remote_orphans(&config.ssh_host, &remote_dir, &allowed)?
    } else {
        0
    };

    Ok(LaneSyncAnalysis {
        config,
        mods,
        pending,
        already_synced,
        already_synced_names,
        total_all,
        remote_count,
        to_delete: to_delete as u32,
        remote_index,
    })
}

pub(super) fn preview_lane_mods(app: &AppHandle, lane: SyncLane) -> ServerSyncLanePreview {
    match analyze_lane_sync(app, lane) {
        Ok(analysis) => {
            let allowed: HashSet<String> = analysis
                .mods
                .iter()
                .filter(|entry| !entry.disabled)
                .filter(|entry| mod_applies_to_lane(lane, &entry.side))
                .map(|entry| entry.filename.clone())
                .collect();
            let remote_dir = remote_dir_for_lane(&analysis.config, lane).expect("lane path checked");
            let orphan_names = if analysis.config.delete_extra_remote_jars {
                remote_orphan_names(&analysis.config.ssh_host, &remote_dir, &allowed).unwrap_or_default()
            } else {
                Vec::new()
            };
            let pending_names: Vec<String> = analysis
                .pending
                .iter()
                .map(|(entry, _)| entry.filename.clone())
                .collect();
            let changes = classify_lane_orphans(
                &analysis.mods,
                lane,
                &pending_names,
                &orphan_names,
            );
            let to_delete_items = delete_items_for_names(&analysis.mods, &changes.delete_names);

            ServerSyncLanePreview {
                ok: true,
                local: analysis.total_all,
                remote: analysis.remote_count,
                to_upload: changes.to_upload,
                already_synced: analysis.already_synced,
                to_delete: changes.to_delete,
                to_update: changes.to_update,
                to_upload_names: changes.upload_names,
                to_update_pairs: changes.update_pairs,
                to_delete_names: changes.delete_names,
                to_delete_items,
                errors: Vec::new(),
            }
        }
        Err(error) => ServerSyncLanePreview {
            ok: false,
            local: 0,
            remote: 0,
            to_upload: 0,
            already_synced: 0,
            to_delete: 0,
            to_update: 0,
            to_upload_names: Vec::new(),
            to_update_pairs: Vec::new(),
            to_delete_names: Vec::new(),
            to_delete_items: Vec::new(),
            errors: vec![error],
        },
    }
}

pub(super) fn sync_lane_mods(app: &AppHandle, state: &ServerSyncState) -> Result<ServerSyncBulkResult, String> {
    match sync_lane_mods_inner(app, state) {
        Ok(result) => Ok(result),
        Err(message) => {
            if !state.snapshot().done {
                state.fail_start(message.clone());
                emit_server_sync_progress(app, state);
            }
            Err(message)
        }
    }
}

fn short_msg(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max {
        trimmed.to_string()
    } else {
        format!("{}…", &trimmed[..max.saturating_sub(1)])
    }
}

fn apply_prune_results(
    state: &ServerSyncState,
    mods: &[crate::mods::ModEntry],
    deleted: usize,
    changes: SyncChangeDetails,
) {
    let delete_items = delete_items_for_names(mods, &changes.delete_names);
    state.set_prune_details(
        deleted as u32,
        changes.to_delete,
        changes.to_update,
        changes.delete_names,
        delete_items,
        changes.update_pairs,
    );
}

fn sync_lane_mods_inner(app: &AppHandle, state: &ServerSyncState) -> Result<ServerSyncBulkResult, String> {
    let lane = state.lane;
    let analysis = match analyze_lane_sync(app, lane) {
        Ok(value) => value,
        Err(message) => {
            state.fail_start(message.clone());
            emit_server_sync_progress(app, state);
            return Err(message);
        }
    };

    let LaneSyncAnalysis {
        config,
        mods,
        pending,
        already_synced,
        already_synced_names,
        total_all,
        to_delete,
        remote_index,
        ..
    } = analysis;

    state.set_checking(total_all);
    emit_server_sync_progress(app, state);

    if state.is_cancelled() {
        state.reset();
        emit_server_sync_progress(app, state);
        return Ok(ServerSyncBulkResult {
            uploaded: 0,
            skipped: 0,
            deleted: 0,
            errors: vec!["Отменено.".to_string()],
        });
    }

    let total = pending.len() as u32;
    let pending_names: Vec<String> = pending
        .iter()
        .map(|(entry, _)| entry.filename.clone())
        .collect();
    if total == 0 && to_delete > 0 && config.delete_extra_remote_jars {
        state.set_pruning(to_delete, already_synced, already_synced_names.clone());
    } else {
        state.begin_upload(total, already_synced, total_all, already_synced_names.clone());
    }
    emit_server_sync_progress(app, state);

    if total_all == 0 {
        let mut errors = Vec::new();
        let mut deleted = 0usize;
        if to_delete > 0 && config.delete_extra_remote_jars {
            state.set_pruning(to_delete, already_synced, already_synced_names.clone());
            emit_server_sync_progress(app, state);
        }
        match prune_lane(&config, lane, &mods, &pending_names) {
            Ok((count, changes)) => {
                deleted = count;
                apply_prune_results(state, &mods, deleted, changes);
            }
            Err(error) => errors.push(error),
        }
        let ok = errors.is_empty();
        state.finish(ok);
        emit_server_sync_progress(app, state);
        return Ok(ServerSyncBulkResult {
            uploaded: 0,
            skipped: already_synced as usize,
            deleted,
            errors,
        });
    }

    let mut uploaded = 0usize;
    let mut skipped = already_synced as usize;
    let mut errors = Vec::new();

    if total == 0 {
        let mut deleted = 0usize;
        if to_delete > 0 && config.delete_extra_remote_jars {
            state.set_pruning(to_delete, skipped as u32, already_synced_names.clone());
            emit_server_sync_progress(app, state);
        }
        match prune_lane(&config, lane, &mods, &pending_names) {
            Ok((count, changes)) => {
                deleted = count;
                apply_prune_results(state, &mods, deleted, changes);
            }
            Err(error) => errors.push(error),
        }
        let ok = errors.is_empty();
        state.finish(ok);
        emit_server_sync_progress(app, state);
        return Ok(ServerSyncBulkResult {
            uploaded: 0,
            skipped,
            deleted,
            errors,
        });
    }

    for (index, (entry, local_path)) in pending.iter().enumerate() {
        if state.is_cancelled() {
            errors.push("Отменено.".to_string());
            break;
        }

        let current = index as u32 + 1;
        state.set_step(current, total, &entry.filename);
        emit_server_sync_progress(app, state);

        match upload_mod_for_lane(
            lane,
            &config,
            local_path,
            &entry.filename,
            &entry.side,
            Some(&remote_index),
        ) {
            Ok(true) => {
                uploaded += 1;
                state.add_result(true, false, &entry.filename, None);
            }
            Ok(false) => {
                skipped += 1;
                state.add_result(false, true, &entry.filename, None);
            }
            Err(error) => {
                let message = short_msg(&format!("{}: {error}", entry.filename), 48);
                errors.push(message.clone());
                state.add_result(false, false, &entry.filename, Some(message));
            }
        }
    }

    let mut deleted = 0usize;
    if !state.is_cancelled() {
        if to_delete > 0 && config.delete_extra_remote_jars {
            state.set_pruning(
                to_delete,
                skipped as u32,
                state.snapshot().skipped_names.clone(),
            );
            emit_server_sync_progress(app, state);
        }
        match prune_lane(&config, lane, &mods, &pending_names) {
            Ok((count, changes)) => {
                deleted = count;
                apply_prune_results(state, &mods, deleted, changes);
            }
            Err(error) => errors.push(error),
        }
    }

    if state.is_cancelled() {
        state.reset();
        emit_server_sync_progress(app, state);
        return Ok(ServerSyncBulkResult {
            uploaded,
            skipped,
            deleted: 0,
            errors: vec!["Отменено.".to_string()],
        });
    }

    let ok = errors.is_empty();
    state.finish(ok);
    emit_server_sync_progress(app, state);

    Ok(ServerSyncBulkResult {
        uploaded,
        skipped,
        deleted,
        errors,
    })
}
