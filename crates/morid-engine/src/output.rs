//! Byte framing precedes UTF-8 decoding. Each pipe owns an independent splitter;
//! no partial read, UTF-8 boundary, or final unterminated fragment is a line loss.
use morid_ext::control::Control;

pub struct Splitter {
    separators: Vec<Vec<u8>>,
    pending: Vec<u8>,
}

impl Splitter {
    pub fn new(separators: &[String]) -> Self {
        Self {
            separators: separators
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| s.as_bytes().to_vec())
                .collect(),
            pending: Vec::new(),
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(bytes);
        let mut lines = Vec::new();
        let mut start = 0;
        let mut cursor = 0;
        while cursor < self.pending.len() {
            let suffix = &self.pending[cursor..];
            // Delay a possible multi-byte separator until the next chunk. Longest
            // matching delimiter wins (e.g. CRLF before CR), independent of order.
            if self
                .separators
                .iter()
                .any(|s| s.len() > suffix.len() && s.starts_with(suffix))
            {
                break;
            }
            let found = self
                .separators
                .iter()
                .filter(|s| suffix.starts_with(s))
                .map(Vec::len)
                .max();
            if let Some(length) = found {
                lines.push(String::from_utf8_lossy(&self.pending[start..cursor]).into_owned());
                cursor += length;
                start = cursor;
            } else {
                cursor += 1;
            }
        }
        self.pending.drain(..start);
        lines
    }

    pub fn finish(&mut self) -> Vec<String> {
        // At EOF, resolve complete separators without waiting for a possible
        // longer separator. Preserve any remaining unterminated fragment.
        if self.pending.is_empty() {
            return Vec::new();
        }
        let mut lines = Vec::new();
        let mut cursor = 0;
        let mut start = 0;
        while cursor < self.pending.len() {
            let suffix = &self.pending[cursor..];
            if let Some(length) = self
                .separators
                .iter()
                .filter(|s| suffix.starts_with(s))
                .map(Vec::len)
                .max()
            {
                lines.push(String::from_utf8_lossy(&self.pending[start..cursor]).into_owned());
                cursor += length;
                start = cursor;
            } else {
                cursor += 1;
            }
        }
        if start < self.pending.len() {
            lines.push(String::from_utf8_lossy(&self.pending[start..]).into_owned());
        }
        self.pending.clear();
        lines
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub percent: Option<f64>,
    pub total_bytes: Option<f64>,
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    pub filename: Option<String>,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Classified {
    Output(String),
    Progress(Progress),
}

pub struct Classifier {
    remove: Vec<String>,
    skip: Vec<String>,
    control: Control,
    pub current_progress: Option<Progress>,
    destination: Option<String>,
}

impl Classifier {
    pub fn new(remove: Vec<String>, skip: Vec<String>, control: Control) -> Self {
        Self {
            remove,
            skip,
            control,
            current_progress: None,
            destination: None,
        }
    }

    pub fn classify(&mut self, input: &str) -> Option<Classified> {
        // ANSI stripping is a reader policy, before extension RemoveText and
        // classification. It never unescapes textual backslashes or splits CR.
        let mut line = strip_terminal_escapes(input);
        for text in &self.remove {
            line = line.replace(text, "");
        }
        if self.skip.iter().any(|text| line.contains(text)) {
            return None;
        }
        if let Some(name) = line.strip_prefix("[download] Destination: ") {
            self.destination = Some(name.to_owned());
        }
        if self.control.matches(&line) {
            if let Some(mut progress) = yt_dlp_progress(&line) {
                if progress.filename.is_none() {
                    progress.filename.clone_from(&self.destination);
                }
                self.current_progress = Some(progress.clone());
                return Some(Classified::Progress(progress));
            }
        }
        Some(Classified::Output(line))
    }
}

/// Strip CSI (including erase-line/color), OSC and ST-terminated terminal
/// strings. Work on characters so non-ASCII filenames are never byte-corrupted.
/// Unterminated escape tails are dropped; printable text outside them survives.
fn strip_terminal_escapes(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for next in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            Some(kind @ (']' | 'P' | 'X' | '^' | '_')) => {
                while let Some(next) = chars.next() {
                    if (kind == ']' && next == '\u{7}')
                        || (next == '\u{1b}' && chars.next_if_eq(&'\\').is_some())
                    {
                        break;
                    }
                }
            }
            Some(ch) if (' '..='/').contains(&ch) => {
                while chars.peek().is_some_and(|next| (' '..='/').contains(next)) {
                    chars.next();
                }
                if chars.peek().is_some_and(|next| ('0'..='~').contains(next)) {
                    chars.next();
                }
            }
            Some(_) | None => {}
        }
    }
    output
}

fn finite_number(input: &str) -> Option<f64> {
    let number = input.parse::<f64>().ok()?;
    (number.is_finite() && number >= 0.0).then_some(number)
}

fn byte_quantity(input: &str) -> Option<f64> {
    let input = input.trim_start_matches('~').trim_end_matches("/s");
    let boundary = input.find(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .unwrap_or(input.len());
    let value = finite_number(&input[..boundary])?;
    let factor = match &input[boundary..] {
        "" | "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0_f64.powi(2),
        "GiB" => 1024.0_f64.powi(3),
        "TiB" => 1024.0_f64.powi(4),
        "PiB" => 1024.0_f64.powi(5),
        "kB" | "KB" => 1000.0,
        "MB" => 1000.0_f64.powi(2),
        "GB" => 1000.0_f64.powi(3),
        "TB" => 1000.0_f64.powi(4),
        _ => return None,
    };
    let bytes = value * factor;
    bytes.is_finite().then_some(bytes)
}

fn clock_eta(input: &str) -> Option<f64> {
    let fields = input.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&fields.len()) {
        return None;
    }
    let mut seconds = 0_u64;
    for (index, field) in fields.into_iter().enumerate() {
        let value = field.parse::<u64>().ok()?;
        if index > 0 && value >= 60 {
            return None;
        }
        seconds = seconds.checked_mul(60)?.checked_add(value)?;
    }
    Some(seconds as f64)
}

/// Only the two named yt-dlp shapes are decoded in Phase 3. A control predicate
/// alone is not sufficient: unrecognized text remains Output, not empty Progress.
fn yt_dlp_progress(line: &str) -> Option<Progress> {
    let body = line.strip_prefix("[download] ")?.trim_start();
    if body.starts_with("downloaded_bytes:") {
        fn number(line: &str, key: &str) -> Option<f64> {
            finite_number(line.split_once(key)?.1.split_whitespace().next()?)
        }
        let downloaded = number(body, "downloaded_bytes:");
        let total_bytes = number(body, "total_bytes:")
            .filter(|value| *value > 0.0)
            .or_else(|| number(body, "total_bytes_estimate:").filter(|value| *value > 0.0));
        let speed = number(body, "progress.speed:");
        let eta = number(body, "ETA:");
        let filename = body.split_once("filename:").map(|(_, name)| name.to_owned())
            .filter(|name| !name.is_empty() && name != "NA" && name != "Unknown");
        if downloaded.is_none() && total_bytes.is_none() && speed.is_none() && eta.is_none() && filename.is_none() {
            return None;
        }
        return Some(Progress {
            percent: downloaded.zip(total_bytes).map(|(done, total)| 100.0 * done / total).filter(|value| value.is_finite()),
            total_bytes,
            speed,
            eta,
            filename,
            line: line.to_owned(),
        });
    }
    let words = body.split_whitespace().collect::<Vec<_>>();
    let percent = finite_number(words.first()?.strip_suffix('%')?)?;
    if words.get(1) != Some(&"of") {
        return None;
    }
    let size_index = if words.get(2) == Some(&"~") { 3 } else { 2 };
    let size = words.get(size_index)?;
    let total_bytes = byte_quantity(size);
    if total_bytes.is_none() && !matches!(*size, "Unknown" | "NA") {
        return None;
    }
    let after = |marker| words.iter().position(|word| *word == marker)
        .and_then(|index| words.get(index + 1).copied());
    Some(Progress {
        percent: Some(percent),
        total_bytes,
        speed: after("at").and_then(byte_quantity),
        eta: after("ETA").and_then(clock_eta),
        filename: None,
        line: line.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(bytes: &[u8], separators: &[String], chunk: usize) -> Vec<String> {
        let mut splitter = Splitter::new(separators);
        let mut lines = Vec::new();
        for bytes in bytes.chunks(chunk) {
            lines.extend(splitter.push(bytes));
        }
        lines.extend(splitter.finish());
        assert!(splitter.finish().is_empty());
        lines
    }

    #[test]
    fn fixture_framing_is_invariant_at_every_chunk_size() {
        for fixture in [
            include_bytes!("../../../fixtures/streams/yt-dlp/partial-line.bin").as_slice(),
            include_bytes!("../../../fixtures/streams/yt-dlp/cr.bin").as_slice(),
            include_bytes!("../../../fixtures/streams/yt-dlp/non-utf8.bin").as_slice(),
        ] {
            for separators in [
                vec!["\n".into()],
                vec!["\r\n".into(), "\n".into(), "\r".into()],
            ] {
                let expected = split(fixture, &separators, fixture.len());
                for size in 1..=fixture.len() {
                    assert_eq!(split(fixture, &separators, size), expected, "chunk {size}");
                }
            }
        }
        let lines = split(
            include_bytes!("../../../fixtures/streams/yt-dlp/partial-line.bin"),
            &["\n".into()],
            1,
        );
        assert!(lines.last().expect("final fragment").ends_with("ETA 00:11"));
        assert_eq!(
            split("é\r\n終\r".as_bytes(), &["\r\n".into(), "\r".into()], 1),
            ["é", "終"]
        );
    }

    #[test]
    fn remove_precedes_skip_and_progress_replaces_output() {
        let control = Control::parse(&serde_json::json!({"lhs":{"startsWith":"[download]"}}))
            .expect("control");
        let mut classifier = Classifier::new(vec!["ESC".into()], vec!["drop".into()], control);
        assert!(classifier.classify("ESCdrop").is_none());
        assert!(matches!(
            classifier.classify("log"),
            Some(Classified::Output(_))
        ));
        let line = "ESC[download] downloaded_bytes:50 ETA:3 total_bytes_estimate:200 total_bytes:100 progress.speed:12 filename:Загрузка video.mp4";
        let Some(Classified::Progress(progress)) = classifier.classify(line) else {
            panic!("expected progress")
        };
        assert_eq!(progress.percent, Some(50.0));
        assert_eq!(progress.total_bytes, Some(100.0));
        assert_eq!(progress.speed, Some(12.0));
        assert_eq!(progress.eta, Some(3.0));
        assert_eq!(progress.filename.as_deref(), Some("Загрузка video.mp4"));
        assert_eq!(classifier.current_progress.as_ref(), Some(&progress));
        classifier.classify("[download] downloaded_bytes:100 total_bytes:100");
        assert_eq!(
            classifier
                .current_progress
                .as_ref()
                .expect("replaced")
                .percent,
            Some(100.0)
        );
    }
    #[test]
    fn both_progress_shapes_are_classified() {
        // Actual progress.txt has console progress only; prepend the template
        // contract sample without modifying any captured fixture bytes.
        let template = "[download] downloaded_bytes:1024 ETA:86 total_bytes_estimate:NA total_bytes:3006209 progress.speed:34781.85 filename:Café movie.mp4\n";
        let mut mixed = template.as_bytes().to_vec();
        mixed.extend_from_slice(include_bytes!("../../../fixtures/streams/yt-dlp/progress.txt"));
        for size in 1..=mixed.len() {
            let control = Control::parse(&serde_json::json!({
                "Connector":"&&", "lhs":{"startsWith":"[download]"}, "rhs":{"contains":"ETA"}
            })).expect("control");
            let mut classifier = Classifier::new(vec![], vec![], control);
            let mut progress = Vec::new();
            for line in split(&mixed, &["\r".into(), "\n".into()], size) {
                if let Some(event) = classifier.classify(&line) {
                    match event {
                        Classified::Progress(value) => progress.push(value),
                        Classified::Output(text) => assert!(!text.contains("ETA"), "progress leaked to output: {text}"),
                    }
                }
            }
            assert_eq!(progress.len(), 6);
            assert_eq!(progress[0].eta, Some(86.0));
            assert_eq!(progress[0].speed, Some(34781.85));
            assert_eq!(progress[0].filename.as_deref(), Some("Café movie.mp4"));
            assert!((progress[0].percent.expect("template percent") - 1024.0 / 3006209.0 * 100.0).abs() < 1e-9);
            for (item, percent) in progress[1..].iter().zip([0.0, 25.0, 50.0, 100.0, 100.0]) {
                assert_eq!(item.percent, Some(percent));
                assert_eq!(item.total_bytes, Some(2097152.0));
                assert_eq!(item.filename.as_deref(), Some("fixture.mp4"));
                assert!(!item.line.contains('\u{1b}'));
            }
            assert_eq!(progress[1].speed, None);
            assert_eq!(progress[1].eta, None);
            assert_eq!(progress[2].speed, Some(4.94 * 1024.0 * 1024.0));
            assert_eq!(progress[2].eta, Some(0.0));
            assert_eq!(classifier.current_progress.as_ref(), progress.last());
        }
    }

    #[test]
    fn ansi_unknowns_and_non_progress_text_are_not_guessed() {
        let control = Control::parse(&serde_json::json!({"lhs":{"startsWith":"[download]"}})).expect("control");
        let mut classifier = Classifier::new(vec![], vec![], control);
        let Some(Classified::Progress(value)) = classifier.classify(
            "\x1b]0;window title\x07\x1b[31m[download] 75% of ~1.5GiB at 3MB/s ETA 01:02:03\x1b[0m"
        ) else { panic!("console progress") };
        assert_eq!(value.total_bytes, Some(1.5 * 1024.0_f64.powi(3)));
        assert_eq!(value.speed, Some(3000000.0));
        assert_eq!(value.eta, Some(3723.0));
        assert!(matches!(classifier.classify("[download] unrelated ETA note"), Some(Classified::Output(_))));
        assert!(matches!(classifier.classify("[download] NaN% of 2MiB ETA 00:00"), Some(Classified::Output(_))));
        assert!(matches!(classifier.classify("[download] Destination: Café.mp4"), Some(Classified::Output(_))));
    }

}
