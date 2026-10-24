//! File names that every filesystem accepts.

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const MAX_CHARS: usize = 150;

/// Makes `name` safe as a file name on Windows, macOS and Linux.
///
/// Characters that are not allowed become underscores, so `AC/DC` becomes `AC_DC`. Trailing
/// dots and spaces (which Windows silently drops) are removed, reserved device names such as
/// `CON` get an underscore in front, and very long names are shortened.
pub fn sanitize(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let shortened: String = replaced.chars().take(MAX_CHARS).collect();
    let trimmed = shortened.trim_matches(|c: char| c == ' ' || c == '.');
    if trimmed.is_empty() {
        return "untitled".to_string();
    }
    // Windows also reserves these names when they have an extension, like `CON.txt`.
    let device = trimmed
        .split('.')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_uppercase();
    if RESERVED.contains(&device.as_str()) {
        format!("_{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// The file name for one track of a CUE album, such as `07 - No Quarter.m4a`.
pub fn track_file_name(number: u32, title: Option<&str>, extension: &str) -> String {
    let title = sanitize(title.unwrap_or("untitled"));
    format!("{number:02} - {title}.{extension}")
}

/// The file name for a single file: its own name with the new extension.
pub fn converted_file_name(stem: &str, extension: &str) -> String {
    format!("{}.{extension}", sanitize(stem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_names_are_untouched() {
        assert_eq!(sanitize("D'yer Mak'er"), "D'yer Mak'er");
        assert_eq!(
            sanitize("The Song Remains the Same"),
            "The Song Remains the Same"
        );
        assert_eq!(sanitize("Café Tacvba (Live) - Ñ"), "Café Tacvba (Live) - Ñ");
    }

    #[test]
    fn forbidden_characters_become_underscores() {
        assert_eq!(sanitize("AC/DC"), "AC_DC");
        assert_eq!(sanitize(r#"a<b>c:d"e\f|g?h*i"#), "a_b_c_d_e_f_g_h_i");
        assert_eq!(sanitize("tab\there"), "tab_here");
    }

    #[test]
    fn trailing_dots_and_spaces_go() {
        assert_eq!(sanitize("Wait... "), "Wait");
        assert_eq!(sanitize("  padded  "), "padded");
    }

    #[test]
    fn empty_names_get_a_placeholder() {
        assert_eq!(sanitize(""), "untitled");
        assert_eq!(sanitize("..."), "untitled");
    }

    #[test]
    fn windows_device_names_are_defused() {
        assert_eq!(sanitize("CON"), "_CON");
        assert_eq!(sanitize("nul"), "_nul");
        assert_eq!(sanitize("com1.backup"), "_com1.backup");
        // Only an exact device name is a problem.
        assert_eq!(sanitize("Console"), "Console");
    }

    #[test]
    fn long_names_are_shortened() {
        let long = "x".repeat(400);
        assert_eq!(sanitize(&long).chars().count(), MAX_CHARS);
    }

    #[test]
    fn track_names_are_numbered_and_titled() {
        assert_eq!(
            track_file_name(7, Some("No Quarter"), "m4a"),
            "07 - No Quarter.m4a"
        );
        assert_eq!(
            track_file_name(12, Some("AC/DC"), "flac"),
            "12 - AC_DC.flac"
        );
        assert_eq!(track_file_name(3, None, "wav"), "03 - untitled.wav");
    }

    #[test]
    fn converted_names_keep_the_stem() {
        assert_eq!(
            converted_file_name("14 - Shot at the Night", "m4a"),
            "14 - Shot at the Night.m4a"
        );
    }
}
