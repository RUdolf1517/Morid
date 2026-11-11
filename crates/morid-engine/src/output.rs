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
}

impl Classifier {
    pub fn new(remove: Vec<String>, skip: Vec<String>, control: Control) -> Self {
        Self {
            remove,
            skip,
            control,
            current_progress: None,
        }
    }

    pub fn classify(&mut self, input: &str) -> Option<Classified> {
        let mut line = input.to_owned();
        for text in &self.remove {
            line = line.replace(text, "");
        }
        if self.skip.iter().any(|text| line.contains(text)) {
            return None;
        }
        if self.control.matches(&line) {
            let progress = yt_dlp_progress(line);
            self.current_progress = Some(progress.clone());
            Some(Classified::Progress(progress))
        } else {
            Some(Classified::Output(line))
        }
    }
}

// Phase 3's named yt-dlp --progress-template contract, not a generic guess at
// human console text. Unknown/NA values remain absent; filename may contain spaces.
fn yt_dlp_progress(line: String) -> Progress {
    fn number(line: &str, key: &str) -> Option<f64> {
        let value = line
            .split_once(key)?
            .1
            .split_whitespace()
            .next()?
            .parse::<f64>()
            .ok()?;
        value.is_finite().then_some(value)
    }
    let downloaded = number(&line, "downloaded_bytes:");
    let total_bytes = number(&line, "total_bytes:")
        .filter(|v| *v > 0.0)
        .or_else(|| number(&line, "total_bytes_estimate:").filter(|v| *v > 0.0));
    Progress {
        percent: downloaded.zip(total_bytes).map(|(done, total)| 100.0 * done / total),
        total_bytes,
        speed: number(&line, "progress.speed:"),
        eta: number(&line, "ETA:"),
        filename: line.split_once("filename:").map(|(_, name)| name.to_owned()),
        line,
    }
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
            for separators in [vec!["\n".into()], vec!["\r\n".into(), "\n".into(), "\r".into()]] {
                let expected = split(fixture, &separators, fixture.len());
                for size in 1..=fixture.len() {
                    assert_eq!(split(fixture, &separators, size), expected, "chunk {size}");
                }
            }
        }
        let lines = split(include_bytes!("../../../fixtures/streams/yt-dlp/partial-line.bin"), &["\n".into()], 1);
        assert!(lines.last().expect("final fragment").ends_with("ETA 00:11"));
        assert_eq!(split("é\r\n終\r".as_bytes(), &["\r\n".into(), "\r".into()], 1), ["é", "終"]);
    }

    #[test]
    fn remove_precedes_skip_and_progress_replaces_output() {
        let control = Control::parse(&serde_json::json!({"lhs":{"startsWith":"[download]"}})).expect("control");
        let mut classifier = Classifier::new(vec!["ESC".into()], vec!["drop".into()], control);
        assert!(classifier.classify("ESCdrop").is_none());
        assert!(matches!(classifier.classify("log"), Some(Classified::Output(_))));
        let line = "ESC[download] downloaded_bytes:50 ETA:3 total_bytes_estimate:200 total_bytes:100 progress.speed:12 filename:Загрузка video.mp4";
        let Some(Classified::Progress(progress)) = classifier.classify(line) else { panic!("expected progress") };
        assert_eq!(progress.percent, Some(50.0));
        assert_eq!(progress.total_bytes, Some(100.0));
        assert_eq!(progress.speed, Some(12.0));
        assert_eq!(progress.eta, Some(3.0));
        assert_eq!(progress.filename.as_deref(), Some("Загрузка video.mp4"));
        assert_eq!(classifier.current_progress.as_ref(), Some(&progress));
        classifier.classify("[download] downloaded_bytes:100 total_bytes:100");
        assert_eq!(classifier.current_progress.as_ref().expect("replaced").percent, Some(100.0));
    }
}
