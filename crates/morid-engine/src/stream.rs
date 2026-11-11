//! Live merge policy: **FIFO of completed-read submissions**, not stdout-first,
//! not one entire pipe followed by the other. Two blocking readers submit at most
//! 4096 bytes per record into one bounded FIFO. Channel insertion is the arrival
//! linearization point. The single consumer processes those records in FIFO
//! order, and emits each record's complete lines in byte order; EOF flushes that
//! pipe's final fragment at its EOF record's position. Sequence numbers are
//! assigned only here, never independently by racing producers.
//!
//! This is deterministic for the observed FIFO history; concurrent submissions
//! are linearized by the channel. It does NOT claim a portable reconstruction of
//! the child's write-syscall chronology across two independent kernel pipes.
//! The handshake integration test establishes a strict happens-before order for
//! alternating writes, so its expected order is independent of thread scheduling.
//!
//! Backpressure: at most 64 records queue up; the OS pipe buffers provide the next
//! bound. This bounds queued chunks, not a single unterminated line retained by
//! the splitter. A consumer that stops reading must cancel/drop the ManagedChild before
//! joining readers. Dropping OutputStream disconnects the queue, not the backend.
use crate::output::{Classified, Classifier, Progress, Splitter};
use morid_ext::control::Control;
use std::collections::VecDeque;
use std::io::{self, Read};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

pub struct Rules {
    pub split_lines_by: Vec<String>,
    pub remove_text: Vec<String>,
    pub skip_line_with_text: Vec<String>,
    pub control: Control,
}

impl Rules {
    pub fn from_extension(extension: &morid_ext::Extension) -> Self {
        Self {
            split_lines_by: extension.metadata.split_lines_by.clone(),
            remove_text: extension.metadata.remove_text.clone(),
            skip_line_with_text: extension.metadata.skip_line_with_text.clone(),
            control: extension.control.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputEvent {
    pub sequence: u64,
    pub stream: Stream,
    pub data: Classified,
}

#[derive(Debug)]
pub struct ReadFailure {
    pub stream: Stream,
    pub source: io::Error,
}

impl std::fmt::Display for ReadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.stream, self.source)
    }
}
impl std::error::Error for ReadFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

enum Record {
    Bytes(Stream, Vec<u8>),
    End(Stream),
    Error(Stream, io::Error),
}

fn reader(mut input: impl Read, stream: Stream, sender: SyncSender<Record>) {
    let mut buffer = [0; 4096];
    loop {
        let record = match input.read(&mut buffer) {
            Ok(0) => Record::End(stream),
            Ok(count) => Record::Bytes(stream, buffer[..count].to_vec()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => Record::Error(stream, error),
        };
        let terminal = !matches!(record, Record::Bytes(..));
        if sender.send(record).is_err() || terminal {
            break;
        }
    }
}

pub struct OutputStream {
    receiver: Receiver<Record>,
    readers: Vec<(Stream, JoinHandle<()>)>,
    stdout: Splitter,
    stderr: Splitter,
    classifier: Classifier,
    pending: VecDeque<Result<OutputEvent, ReadFailure>>,
    sequence: u64,
    disconnected: bool,
}

impl OutputStream {
    pub(crate) fn start(
        stdout: impl Read + Send + 'static,
        stderr: impl Read + Send + 'static,
        rules: Rules,
    ) -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(64);
        let tx = sender.clone();
        let out = std::thread::Builder::new()
            .name("morid-stdout".into())
            .spawn(move || reader(stdout, Stream::Stdout, tx))?;
        let err = std::thread::Builder::new()
            .name("morid-stderr".into())
            .spawn(move || reader(stderr, Stream::Stderr, sender))?;
        Ok(Self {
            receiver,
            readers: vec![(Stream::Stdout, out), (Stream::Stderr, err)],
            stdout: Splitter::new(&rules.split_lines_by),
            stderr: Splitter::new(&rules.split_lines_by),
            classifier: Classifier::new(
                rules.remove_text,
                rules.skip_line_with_text,
                rules.control,
            ),
            pending: VecDeque::new(),
            sequence: 0,
            disconnected: false,
        })
    }

    pub fn current_progress(&self) -> Option<&Progress> {
        self.classifier.current_progress.as_ref()
    }

    fn accept(&mut self, record: Record) {
        let (stream, bytes, failure) = match record {
            Record::Bytes(stream, bytes) => (stream, Some(bytes), None),
            Record::End(stream) => (stream, None, None),
            Record::Error(stream, error) => (stream, None, Some(error)),
        };
        let splitter = match stream {
            Stream::Stdout => &mut self.stdout,
            Stream::Stderr => &mut self.stderr,
        };
        let lines = match bytes {
            Some(bytes) => splitter.push(&bytes),
            None => splitter.finish(),
        };
        for line in lines {
            if let Some(data) = self.classifier.classify(&line) {
                self.pending.push_back(Ok(OutputEvent {
                    sequence: self.sequence,
                    stream,
                    data,
                }));
                self.sequence += 1;
            }
        }
        // Flush bytes already received before reporting the read failure.
        if let Some(source) = failure {
            self.pending.push_back(Err(ReadFailure { stream, source }));
        }
    }
}

impl Iterator for OutputStream {
    type Item = Result<OutputEvent, ReadFailure>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Some(event);
            }
            if self.disconnected {
                return None;
            }
            match self.receiver.recv() {
                Ok(record) => self.accept(record),
                Err(_) => {
                    self.disconnected = true;
                    for (stream, handle) in self.readers.drain(..) {
                        if handle.join().is_err() {
                            self.pending.push_back(Err(ReadFailure {
                                stream,
                                source: io::Error::other("pipe reader panicked"),
                            }));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct Broken(bool);
    impl Read for Broken {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if !self.0 {
                self.0 = true;
                buffer[..4].copy_from_slice(b"tail");
                Ok(4)
            } else {
                Err(io::Error::other("injected read failure"))
            }
        }
    }

    #[test]
    fn read_failure_preserves_final_fragment_and_other_pipe() {
        let rules = Rules {
            split_lines_by: vec!["\n".into()],
            remove_text: vec![],
            skip_line_with_text: vec![],
            control: Control::parse(&serde_json::json!({})).expect("control"),
        };
        let events = OutputStream::start(Broken(false), Cursor::new(b"other\n"), rules)
            .expect("readers")
            .collect::<Vec<_>>();
        let out = events
            .iter()
            .filter(|item| match item {
                Ok(event) => event.stream == Stream::Stdout,
                Err(error) => error.stream == Stream::Stdout,
            })
            .collect::<Vec<_>>();
        assert_eq!(out.len(), 2);
        assert!(
            matches!(out[0], Ok(OutputEvent { data: Classified::Output(text), .. }) if text == "tail")
        );
        assert!(out[1].is_err());
        assert!(events.iter().any(|item| matches!(item, Ok(OutputEvent { stream: Stream::Stderr, data: Classified::Output(text), .. }) if text == "other")));
    }
}
