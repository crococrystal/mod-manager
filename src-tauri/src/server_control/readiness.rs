/// Minecraft writes this when the dedicated server finished loading and accepts players.
pub(crate) const DONE_MARKER: &str = "Done (";

/// Returns true when the last "Done (" in the log appeared after the last boot marker.
pub(crate) fn stdout_indicates_ready(stdout: &str) -> bool {
    clean_powershell_stdout(stdout).eq_ignore_ascii_case("ready")
}

/// Returns a human-readable message when PowerShell emitted CLIXML instead of plain text.
pub(crate) fn clixml_noise_message() -> &'static str {
    "Служебный вывод PowerShell (не ошибка сервера)."
}

pub(crate) fn contains_clixml(text: &str) -> bool {
    text.contains("#< CLIXML") || text.contains("<Objs Version=")
}

/// Strips PowerShell CLIXML serialization noise from SSH stdout/stderr.
pub(crate) fn clean_powershell_stdout(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(idx) = trimmed.find("#< CLIXML") {
        return trimmed[..idx].trim().to_string();
    }
    if trimmed.starts_with("<Objs Version=") {
        return String::new();
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOOT_MARKERS: &[&str] = &[
        "modlauncher running",
        "starting minecraft server on",
        "preparing spawn area",
    ];

    fn is_log_ready(log_tail: &str) -> bool {
        log_session_ready(log_tail).unwrap_or(false)
    }

    fn log_session_ready(log: &str) -> Option<bool> {
        if log.trim().is_empty() {
            return Some(false);
        }

        let lower = log.to_ascii_lowercase();
        let last_done = log.rmatch_indices(DONE_MARKER).next().map(|(index, _)| index)?;
        let last_boot = BOOT_MARKERS
            .iter()
            .flat_map(|marker| lower.rmatch_indices(marker))
            .map(|(index, _)| index)
            .max();

        Some(last_boot.is_none_or(|boot| last_done > boot))
    }

    #[test]
    fn ready_when_done_after_boot_markers() {
        let tail = "[main/INFO] ModLauncher running: args\n\
[Server thread/INFO]: Starting Minecraft server on *:25565\n\
[Server thread/INFO]: Done (12.3s)! For help, type \"help\"";
        assert!(is_log_ready(tail));
    }

    #[test]
    fn not_ready_during_mod_loading() {
        let tail = "[main/INFO] ModLauncher running: args\n\
[modloading-worker/INFO]: loading mods";
        assert!(!is_log_ready(tail));
    }

    #[test]
    fn not_ready_when_done_is_from_previous_boot_in_tail() {
        let tail = "[Server thread/INFO]: Done (1.0s)! For help, type \"help\"\n\
[main/INFO] ModLauncher running: args\n\
[modloading-worker/INFO]: scanning mods";
        assert!(!is_log_ready(tail));
    }

    #[test]
    fn ready_when_done_is_followed_by_many_runtime_lines() {
        let tail = "[main/INFO] ModLauncher running: args\n\
[Server thread/INFO]: Done (12.3s)! For help, type \"help\"\n\
[Server thread/INFO]: Player joined\n\
[Server thread/INFO]: Saving chunks";
        assert!(is_log_ready(tail));
    }

    #[test]
    fn ready_when_only_done_in_short_tail() {
        let tail = "[Server thread/INFO]: Done (5.0s)! For help, type \"help\"";
        assert!(is_log_ready(tail));
    }

    #[test]
    fn stdout_ready_marker() {
        assert!(stdout_indicates_ready("ready\n"));
        assert!(!stdout_indicates_ready(""));
    }

    #[test]
    fn strips_clixml_prefix_from_stdout() {
        assert_eq!(clean_powershell_stdout("ready\n#< CLIXML<Objs"), "ready");
        assert!(stdout_indicates_ready("ready\n#< CLIXML<Objs Version=\"1.1\""));
    }
}
