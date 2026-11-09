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

/// The pages the About window links to, by name. The addresses are fixed here, so nothing the
/// page sends can choose where the browser goes.
const SITE: &str = "https://github.com/sleeperninja/RangeForge";

/// The address for one of the About window's links, or `None` for a name that is not one.
pub fn about_address(which: &str) -> Option<String> {
    Some(match which {
        "kofi" => "https://ko-fi.com/sleeperninja42".to_string(),
        "manual" => format!("{SITE}/blob/dev/MANUAL.md"),
        "faq" => format!("{SITE}/blob/dev/FAQ.md"),
        "notices" => format!("{SITE}/blob/dev/THIRD_PARTY_NOTICES.md"),
        "releases" => format!("{SITE}/releases/latest"),
        "lastfm_key" => "https://www.last.fm/api/account/create".to_string(),
        _ => return None,
    })
}

/// Opens one of the About window's links by name.
pub fn open_about(which: &str) -> Result<(), String> {
    let url =
        about_address(which).ok_or_else(|| "That link is not one the program has.".to_string())?;
    launch(&url)
}

/// Opens `url` in the default browser, if it is a Last.fm music page.
pub fn open(url: &str) -> Result<(), String> {
    if !allowed(url) {
        return Err("That address is not one the program opens.".to_string());
    }
    launch(url)
}

fn launch(url: &str) -> Result<(), String> {
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
    fn the_about_links_are_fixed_names_and_nothing_else_opens() {
        for name in ["kofi", "manual", "faq", "notices", "releases", "lastfm_key"] {
            let url = about_address(name).unwrap();
            assert!(url.starts_with("https://"), "{url}");
            assert!(allowed_chars(&url), "{url}");
        }
        assert!(
            about_address("kofi")
                .unwrap()
                .contains("ko-fi.com/sleeperninja42")
        );
        assert_eq!(about_address("https://evil.example"), None);
        assert_eq!(about_address(""), None);
        assert!(open_about("calc.exe").is_err());
    }

    fn allowed_chars(url: &str) -> bool {
        url.bytes()
            .all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'<' | b'>' | b'^' | b'`' | b'|'))
    }

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
