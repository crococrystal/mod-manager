pub(crate) fn is_emoji_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x1F300..=0x1FAFF).contains(&cp)
        || (0x2600..=0x27BF).contains(&cp)
        || (0x1F600..=0x1F64F).contains(&cp)
        || (0x1F900..=0x1F9FF).contains(&cp)
        || (0x1F1E6..=0x1F1FF).contains(&cp)
        || cp == 0x200D
        || cp == 0xFE0F
}

pub(crate) fn strip_filename_decorations(value: &str) -> String {
    value
        .trim()
        .trim_start_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(ch, '-' | '_' | '+' | '(' | '[' | ')' | ']')
                || is_emoji_char(ch)
        })
        .trim()
        .to_string()
}

pub(crate) fn strip_qualifiers(value: &str) -> String {
    let mut result = String::new();
    let mut depth = 0u32;
    for ch in value.chars() {
        match ch {
            '(' | '[' => depth = depth.saturating_add(1),
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

pub(crate) fn normalized_match_key(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

pub(crate) fn slug_key(value: &str) -> String {
    let mut result = String::new();
    let mut previous_dash = false;
    for ch in value.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            result.push('-');
            previous_dash = true;
        }
    }
    result.trim_matches('-').to_string()
}

pub(crate) fn is_version_or_loader_segment(segment: &str) -> bool {
    let s = segment.trim().to_ascii_lowercase();
    if s.is_empty() {
        return true;
    }
    if matches!(
        s.as_str(),
        "neoforge" | "forge" | "fabric" | "quilt" | "client" | "server" | "universal" | "both"
    ) {
        return true;
    }
    if s.starts_with("mc")
        && s.len() > 2
        && s[2..].chars().all(|ch| ch.is_ascii_digit() || ch == '.')
    {
        return true;
    }
    if s.starts_with('v') && s.len() > 1 && s.chars().nth(1).is_some_and(|ch| ch.is_ascii_digit()) {
        return true;
    }
    if [
        "hotfix", "hotfix2", "beta", "alpha", "rc", "pre", "release", "snapshot", "snap", "patch",
    ]
    .iter()
    .any(|needle| s.contains(needle))
    {
        return true;
    }
    if s.len() <= 16
        && s.chars().any(|ch| ch.is_ascii_digit())
        && s.chars().any(|ch| ch.is_ascii_alphabetic())
        && s.chars().filter(|ch| ch.is_ascii_digit()).count() <= 4
    {
        return true;
    }
    let mut has_digit = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            has_digit = true;
        } else if ch != '.'
            && ch != '-'
            && ch != '+'
            && ch != 'b'
            && ch != 'a'
            && ch != 'r'
            && ch != 'c'
        {
            return false;
        }
    }
    has_digit && s.contains('.')
        || (s.contains('+')
            && s.split('+').all(|part| {
                let part = part.trim();
                part.is_empty()
                    || (part.chars().any(|ch| ch.is_ascii_digit())
                        && part
                            .chars()
                            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+')))
            }))
}

fn is_loader_segment(segment: &str) -> bool {
    matches!(
        segment.trim().to_ascii_lowercase().as_str(),
        "neoforge" | "forge" | "fabric" | "quilt"
    )
}

pub(crate) fn loader_hint_from_filename(filename: &str) -> Option<String> {
    let lowered = filename.trim().to_ascii_lowercase();
    if lowered.contains("fabric-loader") {
        return Some("fabric".to_string());
    }
    if lowered.contains("quilt-loader") {
        return Some("quilt".to_string());
    }
    let stem = filename.trim_end_matches(".jar");
    for segment in stem.split(&['-', '_']) {
        if is_loader_segment(segment) {
            return Some(segment.trim().to_ascii_lowercase());
        }
    }
    None
}

pub(crate) fn minecraft_version_hint_from_filename(filename: &str) -> Option<String> {
    let stem = filename.trim_end_matches(".jar");
    for segment in stem.split(&['-', '_']) {
        if is_minecraft_version_segment(segment) {
            let lowered = segment.trim().to_ascii_lowercase();
            let version = lowered.strip_prefix("mc").unwrap_or(&lowered);
            return Some(version.to_string());
        }
    }
    None
}

fn is_minecraft_version_segment(segment: &str) -> bool {
    let lowered = segment.trim().to_ascii_lowercase();
    let version = lowered.strip_prefix("mc").unwrap_or(&lowered);
    version.starts_with("1.")
        && version.len() <= 12
        && version.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

pub(crate) fn mod_name_tokens(value: &str) -> Vec<String> {
    let clean = strip_qualifiers(&strip_filename_decorations(value));
    let mut parts: Vec<String> = clean
        .split(&['-', '_'][..])
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    while parts.len() > 1
        && is_version_or_loader_segment(parts.last().map(|part| part.as_str()).unwrap_or_default())
    {
        parts.pop();
    }
    parts
}

pub(crate) fn strip_version_suffixes(value: &str) -> String {
    let tokens = mod_name_tokens(value);
    if tokens.is_empty() {
        return strip_qualifiers(&strip_filename_decorations(value));
    }
    if tokens.len() == 1 {
        tokens[0].clone()
    } else {
        tokens.join("-")
    }
}

pub(crate) fn spaced_camel_case(value: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = value.chars().collect();
    for (index, ch) in chars.iter().enumerate() {
        if ch.is_ascii_uppercase()
            && index > 0
            && chars
                .get(index - 1)
                .is_some_and(|previous| previous.is_ascii_lowercase())
        {
            result.push(' ');
        }
        result.push(*ch);
    }
    result.trim().to_string()
}

pub(crate) fn hyphenated_to_spaced(value: &str) -> String {
    value
        .split(&['-', '_'][..])
        .filter(|part| !part.is_empty() && !is_version_or_loader_segment(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_side_prefix(value: &str) -> &str {
    let mut chars = value.chars();
    let Some(prefix) = chars.next() else {
        return value;
    };
    let Some(separator) = chars.next() else {
        return value;
    };
    if separator == '-' && matches!(prefix.to_ascii_lowercase(), 'c' | 's' | 'u') {
        chars.as_str()
    } else {
        value
    }
}

fn title_case_if_lowercase(value: &str) -> String {
    if value.chars().any(|ch| ch.is_ascii_uppercase()) {
        return value.to_string();
    }
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn display_name_from_filename(filename: &str) -> String {
    let stem = filename.trim_end_matches(".jar");
    let without_prefix = strip_side_prefix(stem);
    let stripped = strip_version_suffixes(without_prefix);
    let spaced = hyphenated_to_spaced(&spaced_camel_case(&stripped));
    let clean = title_case_if_lowercase(spaced.trim());
    if clean.is_empty() {
        stem.to_string()
    } else {
        clean
    }
}

pub(crate) fn installed_version_from_filename(filename: &str) -> Option<String> {
    let stem = filename.trim_end_matches(".jar");
    let without_prefix = strip_side_prefix(stem);
    let raw_parts: Vec<&str> = without_prefix
        .split(&['-', '_'][..])
        .filter(|part| !part.trim().is_empty())
        .collect();
    if raw_parts.len() <= 1 {
        return None;
    }

    let mut suffix_start = raw_parts.len();
    while suffix_start > 1 && is_version_or_loader_segment(raw_parts[suffix_start - 1]) {
        suffix_start -= 1;
    }
    if suffix_start == raw_parts.len() {
        return None;
    }

    let mut parts = raw_parts[suffix_start..].to_vec();
    while parts.first().is_some_and(|part| is_loader_segment(part)) {
        parts.remove(0);
    }
    if parts
        .first()
        .is_some_and(|part| is_minecraft_version_segment(part))
    {
        parts.remove(0);
    }

    let version = parts.join("-");
    (!version.trim().is_empty()).then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_strips_side_loader_and_version() {
        assert_eq!(
            display_name_from_filename("C-XaeroZoomout-NeoForge-1.21-2.0.0.jar"),
            "Xaero Zoomout"
        );
    }

    #[test]
    fn display_name_humanizes_lowercase_file_stem() {
        assert_eq!(
            display_name_from_filename("U-open-parties-and-claims-neoforge-1.21.1-0.24.2.jar"),
            "Open Parties And Claims"
        );
    }

    #[test]
    fn installed_version_strips_loader_and_minecraft_version() {
        assert_eq!(
            installed_version_from_filename("C-XaeroZoomout-NeoForge-1.21-2.0.0.jar"),
            Some("2.0.0".to_string())
        );
    }

    #[test]
    fn installed_version_keeps_qualified_suffix() {
        assert_eq!(
            installed_version_from_filename("Alshanex_Familiars-1.21.1_v2.0_HotFix.jar"),
            Some("v2.0-HotFix".to_string())
        );
    }
}
