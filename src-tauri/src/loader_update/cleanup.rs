use std::path::Path;

use super::ssh;

pub(crate) fn is_installer_file_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower.starts_with("neoforge-") && lower.ends_with("-installer.jar")
}

pub(crate) fn remove_installers_in_dir(dir: &Path) -> Result<Vec<String>, String> {
    let mut removed = Vec::new();
    if !dir.is_dir() {
        return Ok(removed);
    }
    for entry in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !is_installer_file_name(name) {
            continue;
        }
        std::fs::remove_file(&path).map_err(|error| error.to_string())?;
        removed.push(name.to_string());
    }
    Ok(removed)
}

pub(crate) fn remove_client_installers(
    libraries_root: &Path,
    instance_root: &Path,
) -> Result<Vec<String>, String> {
    let mut removed = remove_installers_in_dir(instance_root)?;
    removed.extend(remove_installers_in_dir(&instance_root.join("minecraft"))?);

    let neoforge_root = libraries_root
        .join("net")
        .join("neoforged")
        .join("neoforge");
    if neoforge_root.is_dir() {
        for entry in std::fs::read_dir(&neoforge_root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry.path().is_dir() {
                removed.extend(remove_installers_in_dir(&entry.path())?);
            }
        }
    }
    Ok(removed)
}

pub(crate) fn remove_server_installers(host: &str, server_root: &str) -> Result<Vec<String>, String> {
    let ps_root = server_root.replace('/', "\\");
    let cmd = format!(
        "powershell -NoProfile -Command \"\
         Get-ChildItem -LiteralPath '{ps_root}' -Filter 'neoforge-*-installer.jar' -File -ErrorAction SilentlyContinue | \
         ForEach-Object {{ Write-Output $_.Name; Remove-Item -LiteralPath $_.FullName -Force -ErrorAction Stop }}\""
    );
    let output = ssh::ssh_command(host, &cmd)?;
    if !output.status.success() {
        return Err(crate::ssh_util::ssh_command_failed(host, &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_installer_filename() {
        assert!(is_installer_file_name("neoforge-21.1.233-installer.jar"));
        assert!(!is_installer_file_name("neoforge-21.1.233-universal.jar"));
    }
}
