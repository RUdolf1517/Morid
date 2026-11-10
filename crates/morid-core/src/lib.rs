//! Domain values and pure rules. No environmental dependencies.
#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Usage(String),
    UnsupportedUrl(String),
    BackendFailure(String),
    Extension {
        file: String,
        rule: u8,
        message: String,
    },
    Cancelled,
    Destination(String),
    ImportSource(String),
    PartialImport,
    Internal(String),
}

impl Error {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => 1,
            Self::UnsupportedUrl(_) => 2,
            Self::BackendFailure(_) => 3,
            Self::Extension { .. } => 4,
            Self::Cancelled => 5,
            Self::Destination(_) => 6,
            Self::ImportSource(_) => 7,
            Self::PartialImport => 8,
            Self::Internal(_) => 9,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Extension {
                file,
                rule,
                message,
            } => write!(f, "{file}: rule {rule}: {message}"),
            Self::Usage(message)
            | Self::UnsupportedUrl(message)
            | Self::BackendFailure(message)
            | Self::Destination(message)
            | Self::ImportSource(message)
            | Self::Internal(message) => f.write_str(message),
            Self::Cancelled => f.write_str("cancelled"),
            Self::PartialImport => f.write_str("partial import"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url(String);
impl Url {
    pub fn parse(input: &str) -> Result<Self, Error> {
        let valid = input.split_once(':').is_some_and(|(scheme, rest)| {
            !rest.is_empty()
                && scheme
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        });
        if !valid || input.chars().any(char::is_whitespace) {
            return Err(Error::UnsupportedUrl(
                "expected a URL with a scheme".to_owned(),
            ));
        }
        Ok(Self(input.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineId(String);
impl EngineId {
    pub fn parse(name: &str) -> Result<Self, Error> {
        if name.is_empty()
            || name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        {
            return Err(Error::Usage("engine Name must be nonempty and must not contain path separators or control characters".to_owned()));
        }
        Ok(Self(name.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn archive_file(&self) -> String {
        format!("archiveFile-{}.txt", self.0)
    }
}

/// Qt splitCommand's platform-independent double-quote/triple-quote grammar.
/// Single quotes and backslashes are ordinary characters, not POSIX syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOptions(pub Vec<String>);
impl DownloadOptions {
    pub fn parse(input: &str) -> Self {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut quote_count = 0;
        let mut in_quote = false;
        for ch in input.chars() {
            if ch == '"' {
                quote_count += 1;
                if quote_count == 3 {
                    quote_count = 0;
                    word.push(ch);
                }
                continue;
            }
            if quote_count != 0 {
                if quote_count == 1 {
                    in_quote = !in_quote;
                }
                quote_count = 0;
            }
            if !in_quote && ch.is_whitespace() {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            } else {
                word.push(ch);
            }
        }
        if !word.is_empty() {
            words.push(word);
        }
        Self(words)
    }
}

/// The legacy playlist field is an option string, not a numeric-range grammar.
/// See legacy/src/playlistdownloader.cpp:1061–1069. Backend-specific validation
/// belongs to the backend; parsing here is intentionally infallible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistRange {
    original: String,
    tokens: Vec<String>,
}

impl PlaylistRange {
    pub fn parse(input: &str) -> Self {
        Self {
            original: input.to_owned(),
            tokens: DownloadOptions::parse(input).0,
        }
    }

    /// Preserve spelling and whitespace independently of the argument tokens.
    pub fn as_str(&self) -> &str {
        &self.original
    }

    /// No implicit --playlist-items flag, normalization or numerical validation.
    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Format(pub Vec<String>);
impl Format {
    pub fn parse(input: &str) -> Self {
        Self(DownloadOptions::parse(input).0)
    }
}

/// Numeric prefix comparison; wildcard requirements use the explicit D-004 rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version(Vec<u64>);
impl Version {
    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut numbers = Vec::new();
        for component in input.split('.') {
            let prefix: String = component.chars().take_while(char::is_ascii_digit).collect();
            if prefix.is_empty() {
                break;
            }
            numbers.push(
                prefix
                    .parse()
                    .map_err(|_| Error::Usage("version component overflows".to_owned()))?,
            );
            if prefix.len() != component.len() {
                break;
            }
        }
        if numbers.is_empty() {
            return Err(Error::Usage("version must begin with a number".to_owned()));
        }
        while numbers.last() == Some(&0) {
            numbers.pop();
        }
        Ok(Self(numbers))
    }
    pub fn satisfies(requirement: Option<&str>, current: &str) -> Result<bool, Error> {
        let Some(required) = requirement.filter(|s| !s.is_empty()) else {
            return Ok(true);
        };
        if required.split('.').next() == Some("2") {
            return Ok(true);
        }
        Ok(Self::parse(current)? >= Self::parse(required)?)
    }
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (0..self.0.len().max(other.0.len()))
            .map(|i| {
                self.0
                    .get(i)
                    .unwrap_or(&0)
                    .cmp(other.0.get(i).unwrap_or(&0))
            })
            .find(|order| !order.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_variants_map_to_documented_exit_codes() {
        let cases = [
            Error::Usage(String::new()),
            Error::UnsupportedUrl(String::new()),
            Error::BackendFailure(String::new()),
            Error::Extension {
                file: "x".into(),
                rule: 1,
                message: "invalid JSON".into(),
            },
            Error::Cancelled,
            Error::Destination(String::new()),
            Error::ImportSource(String::new()),
            Error::PartialImport,
            Error::Internal(String::new()),
        ];
        for (index, error) in cases.iter().enumerate() {
            assert_eq!(usize::from(error.exit_code()), index + 1);
        }
    }
    #[test]
    fn url_round_trips_byte_for_byte() {
        for input in [
            "https://example.test/v?a=1&b=2#fragment",
            "https://example.test/%2f?q=Загрузка",
            "magnet:?xt=urn:btih:abc",
            "https://例.example/é",
        ] {
            assert_eq!(Url::parse(input).expect("URL").as_str(), input);
        }
        for input in ["", "not a URL", ":abc", "https:"] {
            assert!(Url::parse(input).is_err());
        }
    }
    #[test]
    fn option_tokens_parse_table_driven() {
        for (input, expected) in [
            ("", vec![]),
            ("mp4", vec!["mp4"]),
            ("-f \"best video\"", vec!["-f", "best video"]),
            ("--title \"\"\"quoted\"\"\"", vec!["--title", "\"quoted\""]),
            ("'single quote'", vec!["'single", "quote'"]),
            (r"C:\path\file", vec![r"C:\path\file"]),
            ("\"\"", vec![]),
            ("\"unclosed quote", vec!["unclosed quote"]),
        ] {
            assert_eq!(DownloadOptions::parse(input).0, expected, "{input}");
            assert_eq!(Format::parse(input).0, expected, "format {input}");
        }
    }
    #[test]
    fn playlist_range_preserves_legacy_option_tokens() {
        for (input, expected) in [
            ("", vec![]),
            (" \t ", vec![]),
            ("--playlist-items 2-3", vec!["--playlist-items", "2-3"]),
            (
                "--playlist-items 1-3,7 --lazy-playlist",
                vec!["--playlist-items", "1-3,7", "--lazy-playlist"],
            ),
            (
                "--playlist-items \"1,3,5\" --match-title \"Загрузка видео\"",
                vec![
                    "--playlist-items",
                    "1,3,5",
                    "--match-title",
                    "Загрузка видео",
                ],
            ),
            (
                "  --playlist-start 2   --playlist-end 3  ",
                vec!["--playlist-start", "2", "--playlist-end", "3"],
            ),
        ] {
            let range = PlaylistRange::parse(input);
            assert_eq!(range.as_str(), input);
            assert_eq!(range.tokens(), expected);
            assert_eq!(range.tokens(), DownloadOptions::parse(input).0);
        }
    }

    #[test]
    fn playlist_range_does_not_invent_numeric_refusals() {
        // These are opaque tokens here, not a claim that any backend accepts them.
        for input in [
            "1-3",
            "1,3,5",
            "1-3,7",
            "3-",
            "-3",
            "0",
            "3-1",
            "1,,3",
            "not-a-range",
            "99999999999999999999999999999999999999",
        ] {
            let range = PlaylistRange::parse(input);
            assert_eq!(range.as_str(), input);
            assert_eq!(range.tokens(), [input]);
        }
        // Preserve even incomplete quoting exactly as the shared legacy tokenizer does.
        let input = "--match-title \"unfinished title";
        assert_eq!(
            PlaylistRange::parse(input).tokens(),
            DownloadOptions::parse(input).0
        );
    }

    #[test]
    fn engine_id_archive_name_is_stable() {
        assert_eq!(
            EngineId::parse("yt-dlp").expect("name").archive_file(),
            "archiveFile-yt-dlp.txt"
        );
    }
    #[test]
    fn min_version_2x_is_accepted() {
        for v in ["2.x", "2.0.0", "2.99.0"] {
            assert!(Version::satisfies(Some(v), "0.1.0").expect("version"));
        }
    }
    #[test]
    fn min_version_9x_is_refused() {
        assert!(!Version::satisfies(Some("9.x"), "0.1.0").expect("version"));
    }
    #[test]
    fn numeric_version_comparison_pads_zeroes() {
        assert_eq!(
            Version::parse("1.0").expect("v"),
            Version::parse("1.0.0").expect("v")
        );
        assert!(Version::parse("1.10-rc1").expect("v") > Version::parse("1.9").expect("v"));
        assert!(Version::satisfies(None, "0.1.0").expect("absent"));
    }
}
