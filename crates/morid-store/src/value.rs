//! Qt INI values retain the distinction between QString and QStringList.
//! Port of qt_ini_unescape (qt_ini.py) and escape_qt_ini_value (canonicalise.py).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IniValue {
    String(String),
    StringList(Vec<String>),
}

/// Escape a scalar only. List serialization must call this for each item.
pub fn escape_value(value: &str) -> String {
    let quote = value.contains([';', ',', '=']) || value.starts_with(' ') || value.ends_with(' ');
    let mut out = String::new();
    let mut digit = false;
    for unit in value.encode_utf16() {
        if digit && (0x20..=0x7e).contains(&unit) && (unit as u8).is_ascii_hexdigit() {
            out.push_str(&format!("\\x{unit:x}"));
            continue;
        }
        digit = false;
        match unit {
            0x5c => out.push_str("\\\\"),
            0x22 => out.push_str("\\\""),
            0x20..=0x7e => out.push(char::from(unit as u8)),
            _ => {
                out.push_str(&format!("\\x{unit:x}"));
                digit = true;
            }
        }
    }
    if quote {
        format!("\"{out}\"")
    } else {
        out
    }
}

impl IniValue {
    pub fn encode(&self) -> String {
        match self {
            Self::String(value) => escape_value(value),
            Self::StringList(items) => items
                .iter()
                .map(|item| escape_value(item))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

/// Returns an error for unpaired UTF-16 surrogates rather than replacing bytes.
/// No setting may silently become a replacement character.
pub fn decode_value(text: &str) -> Result<IniValue, std::string::FromUtf16Error> {
    let input: Vec<char> = text.chars().collect();
    let mut units: Vec<u16> = Vec::new();
    let mut items = Vec::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut i = 0;
    fn chop(units: &mut Vec<u16>) {
        while matches!(units.last(), Some(0x20 | 0x09)) {
            units.pop();
        }
    }
    while i < input.len() && matches!(input[i], ' ' | '\t') {
        i += 1;
    }
    while i < input.len() {
        let c = input[i];
        i += 1;
        if c == '\\' {
            if i == input.len() {
                break;
            }
            let escaped = input[i];
            i += 1;
            match escaped {
                'a' => units.push(7),
                'b' => units.push(8),
                'f' => units.push(12),
                'n' => units.push(10),
                'r' => units.push(13),
                't' => units.push(9),
                'v' => units.push(11),
                '"' | '?' | '\'' | '\\' => units.push(escaped as u16),
                'x' | '0'..='7' => {
                    let radix = if escaped == 'x' { 16 } else { 8 };
                    let mut value = if escaped == 'x' {
                        0
                    } else {
                        escaped as u16 - b'0' as u16
                    };
                    let start = i;
                    while i < input.len() {
                        let Some(d) = input[i].to_digit(radix) else {
                            break;
                        };
                        value = value.wrapping_mul(radix as u16).wrapping_add(d as u16);
                        i += 1;
                    }
                    if escaped != 'x' || i > start {
                        units.push(value);
                    }
                }
                '\n' | '\r' => {
                    if i < input.len() && matches!(input[i], '\n' | '\r') && input[i] != escaped {
                        i += 1;
                    }
                }
                _ => {}
            }
        } else if c == '"' {
            quoted = true;
            in_quotes = !in_quotes;
            if !in_quotes {
                while i < input.len() && matches!(input[i], ' ' | '\t') {
                    i += 1;
                }
            }
        } else if c == ',' && !in_quotes {
            if !quoted {
                chop(&mut units);
            }
            items.push(String::from_utf16(&units)?);
            units.clear();
            quoted = false;
            while i < input.len() && matches!(input[i], ' ' | '\t') {
                i += 1;
            }
        } else {
            units.extend(c.encode_utf16(&mut [0; 2]).iter().copied());
        }
    }
    if !quoted {
        chop(&mut units);
    }
    let last = String::from_utf16(&units)?;
    if items.is_empty() {
        Ok(IniValue::String(last))
    } else {
        items.push(last);
        Ok(IniValue::StringList(items))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_history_retains_21_items_and_all_value_bytes() {
        let bytes =
            include_bytes!("../../../fixtures/userdata/media-downloader/settings/settings.ini");
        assert_eq!(bytes.len(), 3351);
        let text = std::str::from_utf8(bytes).expect("ASCII capture");
        let mut out = String::new();
        let mut count = 0;
        let mut lists = 0;
        for line in text.split_inclusive('\n') {
            if let Some((key, raw)) = line.trim_end_matches('\n').split_once('=') {
                let value = decode_value(raw).expect("valid UTF-16");
                if let IniValue::StringList(ref items) = value {
                    lists += 1;
                    assert_eq!(key, "BasicDownloaderOptionsHistory-yt-dlp");
                    assert_eq!(items.len(), 21);
                }
                count += 1;
                out.push_str(key);
                out.push('=');
                out.push_str(&value.encode());
                out.push('\n');
            } else {
                out.push_str(line);
            }
        }
        assert_eq!(count, 94);
        assert_eq!(lists, 1);
        assert_eq!(out.as_bytes(), bytes);
    }

    #[test]
    fn quoted_comma_is_scalar_unquoted_comma_is_list() {
        assert_eq!(
            decode_value("\"a, b\"").expect("scalar"),
            IniValue::String("a, b".into())
        );
        let list = IniValue::StringList(vec!["a, b".into(), "Привет5😀".into(), "".into()]);
        assert_eq!(decode_value(&list.encode()).expect("list"), list);
    }
}

#[cfg(test)]
#[path = "value_corpus.rs"]
mod corpus;

#[cfg(test)]
mod list_tests {
    use super::*;
    #[test]
    fn ini_list_value_round_trips_as_a_list() {
        let list = decode_value("a,b").expect("list");
        assert_eq!(list, IniValue::StringList(vec!["a".into(), "b".into()]));
        assert_eq!(list.encode(), "a, b");
        assert_eq!(decode_value(&list.encode()).expect("round trip"), list);
    }
}
