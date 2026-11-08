//! Opening a web page in the person's own browser.
//!
//! Only Last.fm addresses are ever opened: the address is built by the program from a song's
//! names, and this is the second check that nothing else can be launched through it.

use std::process::Command;

/// Where song tags pages live; the only addresses that may be opened.
const ALLOWED_START: &str = "https://www.last.fm/music/";

/// Whether this is an address the program may open: a Last.fm page, with nothing that could be
/// read as more than one argument or a command.
fn allowed(url: &str) -> bool {
    url.starts_with(ALLOWED_START)
        && url
            .bytes()
            .all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'<' | b'>' | b'^' | b'`' | b'|'))
}

/// Opens `url` in the default browser.
pub fn open(url: &str) -> Result<(), String> {
    if !allowed(url) {
        return Err("That address is not one the program opens.".to_string());
    }
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("rundll32");
        command.args(["url.dll,FileProtocolHandler", url]);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };
    command
        .status()
        .map_err(|error| format!("Could not open the browser: {error}"))
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err("The browser did not open the page.".to_string())
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_last_fm_music_pages_without_odd_characters_are_allowed() {
        assert!(allowed(
            "https://www.last.fm/music/Led+Zeppelin/_/D%27yer+Mak%27er/+tags"
        ));
        assert!(!allowed("https://evil.example/music/x"));
        assert!(!allowed("http://www.last.fm/music/x"));
        assert!(!allowed("https://www.last.fm/music/x y"), "a space");
        assert!(!allowed("https://www.last.fm/music/x\"&calc"), "a quote");
        assert!(!allowed("https://www.last.fm/music/x|calc"), "a pipe");
        assert!(!allowed("file:///C:/Windows/System32/calc.exe"));
    }
}
