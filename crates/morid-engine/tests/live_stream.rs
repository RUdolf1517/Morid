#![forbid(unsafe_code)]

use morid_engine::managed::ManagedChild;
use morid_engine::output::Classified;
use morid_engine::spawn::SpawnSpec;
use morid_engine::stream::{Rules, Stream};
use morid_ext::control::Control;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Files(PathBuf);
impl Files {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos();
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("morid-stream-{}-{nonce}-{sequence}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn start(&self, mode: &str) -> io::Result<ManagedChild> {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/stream_backend.py");
        let spec = SpawnSpec {
            program: if cfg!(windows) { "python".into() } else { "python3".into() },
            args: vec![script.into_os_string(), mode.into(), self.0.clone().into_os_string()],
            cwd: self.0.clone(),
            env: vec![],
        };
        ManagedChild::spawn(&spec).map_err(io::Error::other)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn rules() -> Rules {
    Rules {
        split_lines_by: vec!["\n".into(), "\r".into()],
        remove_text: vec![],
        skip_line_with_text: vec![],
        control: Control::parse(&serde_json::json!({"lhs":{"startsWith":"[download]"}})).expect("control"),
    }
}

#[test]
fn live_interleaving_obeys_acknowledged_write_order() -> io::Result<()> {
    // ACK, not sleep, establishes order; repeat to exercise reader scheduling.
    for _ in 0..4 {
        let files = Files::new()?;
        let mut child = files.start("ordered")?;
        let mut output = child.take_output(rules())?;
        assert!(child.take_output(rules()).is_err(), "cannot take the pipes twice");
        let expected = [Stream::Stderr, Stream::Stdout, Stream::Stderr, Stream::Stdout, Stream::Stderr, Stream::Stdout];
        for (index, stream) in expected.into_iter().enumerate() {
            let event = output.next().expect("backend event").map_err(io::Error::other)?;
            assert_eq!(event.sequence, index as u64);
            assert_eq!(event.stream, stream);
            match (index, &event.data) {
                (0, Classified::Output(text)) => assert_eq!(text, "err-first"),
                (1, Classified::Output(text)) => assert_eq!(text, "out-é-終"),
                (2, Classified::Progress(progress)) => {
                    assert_eq!(progress.total_bytes, Some(3006209.0));
                    assert_eq!(progress.filename.as_deref(), Some("movie.mp4"));
                }
                (3, Classified::Progress(progress)) => {
                    assert_eq!(progress.percent, Some(25.0));
                    assert_eq!(progress.total_bytes, Some(2.0 * 1024.0 * 1024.0));
                    assert_eq!(progress.eta, Some(2.0));
                    assert!(!progress.line.contains('\u{1b}'));
                }
                (4, Classified::Output(text)) => assert_eq!(text, "err-tail"),
                (5, Classified::Output(text)) => assert_eq!(text, "out-tail"),
                _ => panic!("unexpected live event: {event:?}"),
            }
            // No next write until this event was received, framed and checked.
            fs::write(files.0.join(format!("ack-{index}")), b"ack")?;
        }
        assert!(output.next().is_none());
        assert_eq!(output.current_progress().expect("current progress").percent, Some(25.0));
        println!("LIVE_INTERLEAVE_OK: stderr, stdout, stderr-progress, stdout-progress, stderr-EOF-tail, stdout-EOF-tail");
    }
    Ok(())
}

#[test]
fn live_both_pipes_larger_than_os_buffers_are_drained_without_loss() -> io::Result<()> {
    let files = Files::new()?;
    let mut child = files.start("burst")?;
    let output = child.take_output(rules())?;
    let mut seen = [0, 0];
    for (sequence, event) in output.enumerate() {
        let event = event.map_err(io::Error::other)?;
        assert_eq!(event.sequence, sequence as u64);
        let (index, prefix) = match event.stream {
            Stream::Stdout => (0, "O"),
            Stream::Stderr => (1, "E"),
        };
        assert_eq!(event.data, Classified::Output(format!("{prefix}:{}:{}", seen[index], "x".repeat(4090))));
        seen[index] += 1;
    }
    assert_eq!(seen, [512, 512]);
    Ok(())
}

#[test]
fn cancelling_with_live_readers_closes_both_pipes_and_joins() -> io::Result<()> {
    let files = Files::new()?;
    let mut child = files.start("idle")?;
    let mut output = child.take_output(rules())?;
    let ready = output.next().expect("ready event").map_err(io::Error::other)?;
    assert_eq!(ready.stream, Stream::Stderr);
    assert_eq!(ready.data, Classified::Output("ready".into()));
    child.cancel_observed(Duration::from_secs(15))?;
    assert!(output.next().is_none());
    Ok(())
}
