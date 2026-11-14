//! Key dialect ported from canonicalise.py and qt_ini.py, independently of values.

pub fn escape_key(key: &str) -> String {
    let mut out = String::new();
    for unit in key.encode_utf16() {
        match unit {
            0x2f => out.push('\\'),
            0x30..=0x39 | 0x41..=0x5a | 0x61..=0x7a | 0x5f | 0x2d | 0x2e => {
                out.push(char::from(unit as u8));
            }
            0..=0xff => out.push_str(&format!("%{unit:02X}")),
            _ => out.push_str(&format!("%U{unit:04X}")),
        }
    }
    out
}

/// Malformed percent escapes are refused (the deliberate D-016 divergence).
pub fn unescape_key(key: &str) -> Result<String, String> {
    let chars: Vec<char> = key.chars().collect();
    let mut units = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                units.push(0x2f);
                i += 1;
            }
            '%' => {
                let wide = chars.get(i + 1) == Some(&'U');
                let start = i + if wide { 2 } else { 1 };
                let end = start + if wide { 4 } else { 2 };
                let digits = chars
                    .get(start..end)
                    .ok_or_else(|| format!("truncated key escape at {i}"))?;
                if !digits.iter().all(char::is_ascii_hexdigit) {
                    return Err(format!("malformed key escape at {i}"));
                }
                let mut unit = 0u16;
                for digit in digits {
                    unit = unit * 16 + digit.to_digit(16).ok_or("invalid hex")? as u16;
                }
                units.push(unit);
                i = end;
            }
            c => {
                units.extend(c.encode_utf16(&mut [0; 2]).iter().copied());
                i += 1;
            }
        }
    }
    String::from_utf16(&units).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ini_key_round_trips_qt_corpus() {
        for key in [
            "",
            "safe-_.019",
            "a/b",
            "a\\b",
            "Загрузки",
            "😀",
            "%",
            "a b;=",
            "é",
            "\0\n",
        ] {
            assert_eq!(unescape_key(&escape_key(key)).expect("key"), key);
        }
        assert_eq!(escape_key("😀"), "%UD83D%UDE00");
        assert_eq!(unescape_key("%D0%BF").expect("Latin-1"), "Ð¿");
        assert_eq!(unescape_key("%e9").expect("lower hex"), "é");
        for invalid in ["%", "%0", "%zz", "%u0417", "%U123", "%UD800"] {
            assert!(unescape_key(invalid).is_err(), "{invalid}");
        }
    }
}
