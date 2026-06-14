use std::{
    collections::HashSet,
    fs,
    path::Path,
    process::Command,
};

use crate::{
    mod_names::{normalized_match_key, strip_filename_decorations, strip_version_suffixes},
    mods::side_runs_on_server,
    settings::ServerSyncSettings,
};

use super::config::{clean, clean_remote_dir, join_remote_path, normalize_remote_path};

pub(super) fn powershell_literal(path: &str) -> String {
    normalize_remote_path(path).replace('\'', "''")
}

pub(super) struct RemoteDirIndex {
    pub files: std::collections::HashMap<String, u64>,
}

fn ssh_control_path(host: &str) -> std::path::PathBuf {
    let safe: String = host
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    std::env::temp_dir().join(format!("mod-manager-ssh-{safe}.sock"))
}

pub(super) fn ssh_command(host: &str, remote_command: &str) -> Result<std::process::Output, String> {
    let control_path = format!("ControlPath={}", ssh_control_path(host).display());
    Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ControlMaster=auto",
            "-o",
            control_path.as_str(),
            "-o",
            "ControlPersist=120",
            "-o",
            "ConnectTimeout=15",
        ])
        .arg(host)
        .arg(remote_command)
        .output()
        .map_err(|error| crate::ssh_util::ssh_spawn_error(host, error))
}

fn scp_upload(host: &str, local_path: &Path, remote_file: &str) -> Result<(), String> {
    let remote = format!("{host}:{}", normalize_remote_path(remote_file));
    let control_path = format!("ControlPath={}", ssh_control_path(host).display());
    let output = Command::new("scp")
        .args([
            "-q",
            "-o",
            "BatchMode=yes",
            "-o",
            "ControlMaster=auto",
            "-o",
            control_path.as_str(),
            "-o",
            "ControlPersist=120",
            "-o",
            "ConnectTimeout=60",
        ])
        .arg(local_path)
        .arg(remote)
        .output()
        .map_err(|error| crate::ssh_util::ssh_spawn_error(host, error))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("No such file or directory") {
        return Err("Папка не найдена.".to_string());
    }
    Err(crate::ssh_util::ssh_command_failed(host, &output))
}

pub(super) fn upload_remote_file(host: &str, local_path: &Path, remote_file: &str) -> Result<(), String> {
    scp_upload(host, local_path, remote_file)
}

fn format_index_dir_error(host: &str, remote_dir: &str, stderr: &str) -> String {
    if !stderr.is_empty() {
        return crate::ssh_util::explain_ssh_error(host, stderr);
    }
    let normalized = remote_dir.to_ascii_lowercase();
    if normalized.contains(".ssh/config") || normalized.ends_with("/config") {
        return "Не папка mods.".to_string();
    }
    "Папка недоступна.".to_string()
}

pub(super) fn index_remote_dir(host: &str, remote_dir: &str) -> Result<RemoteDirIndex, String> {
    let path = powershell_literal(remote_dir);
    let cmd = format!(
        "powershell -NoProfile -Command \"Get-ChildItem -LiteralPath '{}' -Filter *.jar -File -ErrorAction SilentlyContinue | ForEach-Object {{ Write-Output ($_.Name + '|' + $_.Length) }}\"",
        path
    );
    let output = ssh_command(host, &cmd)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format_index_dir_error(host, remote_dir, &stderr));
    }

    let mut files = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, size_raw)) = line.rsplit_once('|') else {
            continue;
        };
        let Ok(size) = size_raw.trim().parse::<u64>() else {
            continue;
        };
        if !name.is_empty() {
            files.insert(name.to_string(), size);
        }
    }
    Ok(RemoteDirIndex { files })
}

pub(super) fn remote_file_matches(index: &RemoteDirIndex, filename: &str, local_size: u64) -> bool {
    index
        .files
        .get(filename)
        .map(|size| *size == local_size)
        .unwrap_or(false)
}

pub(super) fn list_remote_jars(host: &str, remote_dir: &str) -> Result<Vec<String>, String> {
    Ok(index_remote_dir(host, remote_dir)?
        .files
        .into_keys()
        .collect())
}

fn delete_remote_file(host: &str, remote_file: &str) -> Result<(), String> {
    delete_remote_files(host, &[remote_file.to_string()])?;
    Ok(())
}

fn delete_remote_files(host: &str, remote_files: &[String]) -> Result<usize, String> {
    if remote_files.is_empty() {
        return Ok(0);
    }

    const BATCH: usize = 50;
    let mut deleted = 0usize;

    for chunk in remote_files.chunks(BATCH) {
        let paths = chunk
            .iter()
            .map(|file| format!("'{}'", powershell_literal(file)))
            .collect::<Vec<_>>()
            .join(",");
        let cmd = format!(
            "powershell -NoProfile -Command \"Remove-Item -LiteralPath {paths} -Force -ErrorAction SilentlyContinue\""
        );
        let output = ssh_command(host, &cmd)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.contains("Cannot find path") || stderr.contains("does not exist") {
                deleted += chunk.len();
                continue;
            }
            return Err(if stderr.is_empty() {
                "Не удалено.".to_string()
            } else {
                stderr
            });
        }
        deleted += chunk.len();
    }

    Ok(deleted)
}

fn rename_remote_file(host: &str, remote_file: &str, new_name: &str) -> Result<(), String> {
    let file_literal = powershell_literal(remote_file);
    let name_literal = powershell_literal(new_name);
    let cmd = format!(
        "powershell -NoProfile -Command \"if (Test-Path -LiteralPath '{file_literal}') {{ Rename-Item -LiteralPath '{file_literal}' -NewName '{name_literal}' -Force }}\""
    );
    let output = ssh_command(host, &cmd)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("Cannot find path") || stderr.contains("does not exist") {
        return Ok(());
    }
    Err(if stderr.is_empty() {
        "Не удалось переименовать файл на сервере.".to_string()
    } else {
        stderr
    })
}

pub(super) fn disable_remote_mod(config: &ServerSyncSettings, filename: &str) -> Result<(), String> {
    let disabled_name = format!("{filename}.disable");
    if let Some(dir) = clean_remote_dir(&config.server_mods_path) {
        let remote = join_remote_path(&dir, filename);
        rename_remote_file(&config.ssh_host, &remote, &disabled_name)?;
    }
    if let Some(dir) = clean_remote_dir(&config.distribution_mods_path) {
        let remote = join_remote_path(&dir, filename);
        rename_remote_file(&config.ssh_host, &remote, &disabled_name)?;
    }
    Ok(())
}

pub(super) fn enable_remote_mod(config: &ServerSyncSettings, filename: &str) -> Result<(), String> {
    let disabled_name = format!("{filename}.disable");
    if let Some(dir) = clean_remote_dir(&config.server_mods_path) {
        let remote = join_remote_path(&dir, &disabled_name);
        rename_remote_file(&config.ssh_host, &remote, filename)?;
    }
    if let Some(dir) = clean_remote_dir(&config.distribution_mods_path) {
        let remote = join_remote_path(&dir, &disabled_name);
        rename_remote_file(&config.ssh_host, &remote, filename)?;
    }
    Ok(())
}

pub(super) fn delete_remote_jar(config: &ServerSyncSettings, side: &str, filename: &str) -> Result<(), String> {
    if side_runs_on_server(side) {
        if let Some(dir) = clean_remote_dir(&config.server_mods_path) {
            let remote = join_remote_path(&dir, filename);
            delete_remote_file(&config.ssh_host, &remote)?;
        }
    } else if let Some(dir) = clean_remote_dir(&config.server_mods_path) {
        let remote = join_remote_path(&dir, filename);
        let _ = delete_remote_file(&config.ssh_host, &remote);
    }
    if let Some(dir) = clean_remote_dir(&config.distribution_mods_path) {
        let remote = join_remote_path(&dir, filename);
        delete_remote_file(&config.ssh_host, &remote)?;
    }
    Ok(())
}

pub(super) fn prune_remote_orphans(
    host: &str,
    remote_dir: &str,
    allowed: &HashSet<String>,
) -> Result<usize, String> {
    let to_delete: Vec<String> = list_remote_jars(host, remote_dir)?
        .into_iter()
        .filter(|name| !allowed.contains(name))
        .map(|name| join_remote_path(remote_dir, &name))
        .collect();
    delete_remote_files(host, &to_delete)
}

pub(super) fn upload_mod(
    config: &ServerSyncSettings,
    local_path: &Path,
    filename: &str,
    side: &str,
    previous_filename: Option<&str>,
    server_index: Option<&RemoteDirIndex>,
    distribution_index: Option<&RemoteDirIndex>,
) -> Result<(bool, bool), String> {
    let local_size = fs::metadata(local_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut server_uploaded = false;
    let mut distribution_uploaded = false;

    if side_runs_on_server(side) {
        if let Some(dir) = clean_remote_dir(&config.server_mods_path) {
            let needs_upload = server_index
                .map(|index| !remote_file_matches(index, filename, local_size))
                .unwrap_or(true);
            if needs_upload {
                let remote = join_remote_path(&dir, filename);
                scp_upload(&config.ssh_host, local_path, &remote)?;
                server_uploaded = true;
            }
        }
    }

    if let Some(dir) = clean_remote_dir(&config.distribution_mods_path) {
        let needs_upload = distribution_index
            .map(|index| !remote_file_matches(index, filename, local_size))
            .unwrap_or(true);
        if needs_upload {
            let remote = join_remote_path(&dir, filename);
            scp_upload(&config.ssh_host, local_path, &remote)?;
            distribution_uploaded = true;
        }
    }

    if let Some(old_filename) = previous_filename.and_then(clean) {
        if old_filename != filename {
            delete_remote_jar(config, side, &old_filename)?;
        }
    }

    Ok((server_uploaded, distribution_uploaded))
}

pub(super) fn remote_orphan_names(
    host: &str,
    remote_dir: &str,
    allowed: &HashSet<String>,
) -> Result<Vec<String>, String> {
    Ok(list_remote_jars(host, remote_dir)?
        .into_iter()
        .filter(|name| !allowed.contains(name))
        .collect())
}

pub(super) fn count_remote_orphans(
    host: &str,
    remote_dir: &str,
    allowed: &HashSet<String>,
) -> Result<usize, String> {
    Ok(remote_orphan_names(host, remote_dir, allowed)?.len())
}
