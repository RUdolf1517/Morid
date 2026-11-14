//! Ordered Qt INI document. Values are decoded, never flattened into strings.
use crate::key::{escape_key, unescape_key};
use crate::value::{decode_value, IniValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub section: String,
    pub key: String,
    pub value: IniValue,
    raw_key: String,
    raw_value: String,
}

impl Entry {
    /// Construct a new setting without an input document. Readers retain raw
    /// spelling; new/edited values use the independent Qt encoders.
    pub fn new(section: impl Into<String>, key: impl Into<String>, value: IniValue) -> Self {
        let key = key.into();
        Self {
            section: section.into(),
            raw_key: escape_key(&key),
            raw_value: value.encode(),
            key,
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Section(String),
    Entry(Entry),
    Trivia(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    lines: Vec<Line>,
    final_newline: bool,
    line_ending: String,
}

impl Document {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if !bytes.is_ascii() {
            return Err(if std::str::from_utf8(bytes).is_ok() {
                "finding: non-ASCII UTF-8 INI needs explicit encoding review"
            } else {
                "finding: non-UTF-8 INI; Latin-1 fallback refused"
            }
            .into());
        }
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let mut lines = Vec::new();
        let mut section = String::new();
        for (number, raw) in text.lines().enumerate() {
            let line = raw.trim_start_matches([' ', '\t']);
            if line.is_empty() || (line.starts_with(';') && !line.contains('=')) {
                lines.push(Line::Trivia(raw.into()));
                continue;
            }
            if let Some(header) = line.strip_prefix('[') {
                let end = header
                    .find(']')
                    .ok_or_else(|| format!("line {}: unterminated section", number + 1))?;
                section = unescape_key(header[..end].trim())?;
                lines.push(Line::Section(section.clone()));
                continue;
            }
            let mut quoted = false;
            let split = line
                .char_indices()
                .find_map(|(i, c)| {
                    if c == '"' {
                        quoted = !quoted;
                    }
                    (c == '=' && !quoted).then_some(i)
                })
                .ok_or_else(|| format!("line {}: missing =", number + 1))?;
            let key = unescape_key(line[..split].trim_end_matches([' ', '\t']))?;
            if key.is_empty() || section.is_empty() {
                return Err(format!("line {}: empty key or section", number + 1));
            }
            let value = decode_value(&line[split + 1..])
                .map_err(|e| format!("line {}: {e}", number + 1))?;
            lines.push(Line::Entry(Entry {
                section: section.clone(),
                key,
                value,
                raw_key: line[..split].trim_end_matches([' ', '\t']).into(),
                raw_value: line[split + 1..].into(),
            }));
        }
        Ok(Self {
            lines,
            final_newline: text.ends_with('\n'),
            line_ending: if text.contains("\r\n") { "\r\n" } else { "\n" }.into(),
        })
    }

    pub fn from_entries(entries: Vec<Entry>) -> Self {
        let mut lines = Vec::new();
        let mut sections = Vec::new();
        for entry in &entries {
            if !sections.contains(&entry.section) {
                sections.push(entry.section.clone());
            }
        }
        for section in sections {
            lines.push(Line::Section(section.clone()));
            lines.extend(
                entries
                    .iter()
                    .filter(|entry| entry.section == section)
                    .cloned()
                    .map(Line::Entry),
            );
        }
        Self {
            lines,
            final_newline: true,
            line_ending: "\n".into(),
        }
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.lines.iter().filter_map(|line| {
            if let Line::Entry(entry) = line {
                Some(entry)
            } else {
                None
            }
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = self
            .lines
            .iter()
            .map(|line| match line {
                Line::Section(section) => format!("[{}]", escape_key(section)),
                Line::Entry(entry) => {
                    format!(
                        "{}={}",
                        if unescape_key(&entry.raw_key).as_ref() == Ok(&entry.key) {
                            entry.raw_key.clone()
                        } else {
                            escape_key(&entry.key)
                        },
                        if decode_value(&entry.raw_value).is_ok_and(|value| value == entry.value) {
                            entry.raw_value.clone()
                        } else {
                            entry.value.encode()
                        }
                    )
                }
                Line::Trivia(raw) => raw.clone(),
            })
            .collect::<Vec<_>>()
            .join(&self.line_ending);
        if self.final_newline {
            out.push_str(&self.line_ending);
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_ini_from_fixture_decodes_without_loss() {
        let bytes =
            include_bytes!("../../../fixtures/userdata/media-downloader/settings/settings.ini");
        let document = Document::parse(bytes).expect("fixture");
        assert_eq!(document.entries().count(), 94);
        let lists: Vec<_> = document
            .entries()
            .filter_map(|entry| match &entry.value {
                IniValue::StringList(items) => Some((&entry.key, items)),
                _ => None,
            })
            .collect();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].0, "BasicDownloaderOptionsHistory-yt-dlp");
        assert_eq!(lists[0].1.len(), 21);
        assert_eq!(document.encode().len(), 3351);
        assert_eq!(document.encode(), bytes);
        println!("settings.ini: 94 decoded keys, 21 history items, 3351 bytes: byte-identical");
    }
    #[test]
    fn new_settings_can_be_written_without_a_seed_file() {
        let entry = Entry::new(
            "General",
            "😀/Загрузки",
            IniValue::StringList(vec!["one, quoted".into(), "Отпуск2024".into()]),
        );
        let document = Document::from_entries(vec![entry.clone()]);
        let bytes = document.encode();
        assert!(bytes.is_ascii());
        let decoded = Document::parse(&bytes).expect("new setting");
        let actual = decoded.entries().next().expect("entry");
        assert_eq!(actual.key, entry.key);
        assert_eq!(actual.value, entry.value);
    }

    #[test]
    fn ck_reports_a_utf8_value_instead_of_decoding_it() {
        assert!(Document::parse("[General]\nx=Привет\n".as_bytes())
            .expect_err("finding")
            .contains("UTF-8"));
        assert!(Document::parse(b"[General]\nx=\xff\n")
            .expect_err("finding")
            .contains("Latin-1 fallback refused"));
    }
}
