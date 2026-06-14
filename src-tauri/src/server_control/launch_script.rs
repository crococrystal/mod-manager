use std::fs;
use std::path::PathBuf;

use crate::server_control::os::RemoteOs;
use crate::ssh_exec::{scp_download, scp_upload};

pub(crate) fn validate_script_leaf(script: &str) -> Result<String, String> {
    let trimmed = script.trim();
    if trimmed.is_empty() {
        return Err("Укажите скрипт запуска.".to_string());
    }
    let leaf = trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed);
    if leaf.is_empty() || leaf.contains("..") {
        return Err("Недопустимое имя скрипта.".to_string());
    }
    if leaf.chars().any(|ch| matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err("Имя скрипта должно быть файлом в корне сервера.".to_string());
    }
    Ok(leaf.to_string())
}

fn normalize_remote_path(value: &str) -> String {
    let mut trimmed = value.trim();
    while trimmed.len() >= 2 {
        let starts = trimmed.starts_with('"') || trimmed.starts_with('\'');
        let ends = trimmed.ends_with('"') || trimmed.ends_with('\'');
        if starts && ends {
            trimmed = trimmed[1..trimmed.len() - 1].trim();
        } else {
            break;
        }
    }
    trimmed.replace('\\', "/")
}

pub(crate) fn script_remote_path(server_root: &str, script: &str) -> Result<String, String> {
    let leaf = validate_script_leaf(script)?;
    let base = normalize_remote_path(server_root).trim_end_matches('/').to_string();
    Ok(format!("{base}/{leaf}"))
}

fn temp_file(host: &str, leaf: &str, suffix: &str) -> PathBuf {
    let safe_host: String = host
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let safe_leaf: String = leaf
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    std::env::temp_dir().join(format!("mod-manager-launch-{safe_host}-{safe_leaf}-{suffix}"))
}

pub(crate) fn read_launch_script(
    host: &str,
    _os: RemoteOs,
    server_root: &str,
    script: &str,
) -> Result<String, String> {
    let leaf = validate_script_leaf(script)?;
    let remote = script_remote_path(server_root, &leaf)?;
    let temp = temp_file(host, &leaf, "read");
    if let Err(error) = scp_download(host, &remote, &temp) {
        return Err(if error.contains("No such file") || error.contains("not found") {
            "Скрипт запуска не найден в корне сервера.".to_string()
        } else {
            error
        });
    }
    let content = fs::read_to_string(&temp).map_err(|error| format!("Не удалось прочитать скрипт: {error}"))?;
    let _ = fs::remove_file(&temp);
    Ok(content)
}

pub(crate) fn write_launch_script(
    host: &str,
    _os: RemoteOs,
    server_root: &str,
    script: &str,
    content: &str,
) -> Result<(), String> {
    let leaf = validate_script_leaf(script)?;
    let remote = script_remote_path(server_root, &leaf)?;
    let temp = temp_file(host, &leaf, "write");
    fs::write(&temp, content.as_bytes())
        .map_err(|error| format!("Не удалось подготовить файл: {error}"))?;
    let result = scp_upload(host, &temp, &remote);
    let _ = fs::remove_file(&temp);
    result
}
