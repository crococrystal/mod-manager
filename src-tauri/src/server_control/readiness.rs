/// Minecraft writes this when the dedicated server finished loading and accepts players.
const DONE_MARKER: &str = "Done (";

const BOOT_MARKERS: &[&str] = &[
    "modlauncher running",
    "starting minecraft server on",
    "preparing spawn area",
];

/// Returns true when the tail of `latest.log` indicates the current boot reached "Done".
pub(crate) fn is_log_ready(log_tail: &str) -> bool {
    if !log_tail.contains(DONE_MARKER) {
        return false;
    }

    let mut last_boot_line: Option<usize> = None;
    let mut last_done_line: Option<usize> = None;

    for (index, line) in log_tail.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        if BOOT_MARKERS.iter().any(|marker| lower.contains(marker)) {
            last_boot_line = Some(index);
        }
        if line.contains(DONE_MARKER) {
            last_done_line = Some(index);
        }
    }

    match (last_boot_line, last_done_line) {
        (_, Some(done)) => last_boot_line.is_none_or(|boot| done > boot),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_when_done_after_boot_markers() {
        let tail = "[main/INFO] ModLauncher running: args\n\
[Server thread/INFO]: Starting Minecraft server on *:25565\n\
[Server thread/INFO]: Done (12.3s)! For help, type \"help\"";
        assert!(is_log_ready(tail));
    }

    #[test]
    fn not_ready_during_mod_loading() {
        let tail = r"[main/INFO] ModLauncher running: args
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
    fn ready_when_only_done_in_short_tail() {
        let tail = "[Server thread/INFO]: Done (5.0s)! For help, type \"help\"";
        assert!(is_log_ready(tail));
    }
}
