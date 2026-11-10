//! Unix process-group primitive. No shell, session hook or platform handle escapes.
#![allow(unsafe_code)]

use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

/// Configure a new group before spawning. Rust performs setpgid in the child.
/// A separate session is unnecessary for signalling this group.
pub fn configure(command: &mut Command) -> &mut Command {
    command.process_group(0)
}

/// Borrowing the child prevents its owner from reaping/dropping it while this
/// signal target exists. An exited but unreaped leader still reserves its PID.
/// Use only for a child started with `configure` that has never been reaped
/// (including by `try_wait`). The reference never owns/reaps it.
pub struct ProcessGroup<'child> {
    child: &'child Child,
}

impl<'child> ProcessGroup<'child> {
    pub fn for_child(child: &'child Child) -> Self {
        Self { child }
    }

    fn id(&self) -> libc::pid_t {
        // std's Unix Child::id is the kernel's positive pid_t converted to u32;
        // this conversion reverses it, not an arbitrary user-supplied number.
        self.child.id() as libc::pid_t
    }

    /// Send an unignorable termination signal to the entire group. A successful
    /// signal is NOT proof that the members have exited; the supervisor must wait.
    pub fn kill_tree(&self) -> io::Result<()> {
        // SAFETY: the borrowed Child pins the unreaped leader's positive PID;
        // killpg takes integer values only, and SIGKILL is a valid signal.
        let result = unsafe { libc::killpg(self.id(), libc::SIGKILL) };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }

    /// Probe the group without delivering a signal. EPERM means it still exists.
    pub fn is_present(&self) -> io::Result<bool> {
        // SAFETY: the Child borrow prevents reaping/PID reuse; signal zero is
        // the POSIX existence/permission probe and dereferences no memory.
        let result = unsafe { libc::killpg(self.id(), 0) };
        if result == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::ESRCH) => Ok(false),
            Some(libc::EPERM) => Ok(true),
            _ => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn configured_group_can_be_probed_and_killed() -> io::Result<()> {
        // Start this test harness itself: no external shell or network fixture.
        let mut command = Command::new(std::env::current_exe()?);
        command.args(["--ignored", "--exact", "process_group::tests::group_child"]);
        let mut child = configure(&mut command).spawn()?;
        let (probe, killed) = {
            let group = ProcessGroup::for_child(&child);
            (group.is_present(), group.kill_tree())
        };
        // The borrow ends before wait; reaping while a signal target lives is
        // deliberately rejected by Rust's borrow checker.
        if killed.is_err() {
            let _ = child.kill();
        }
        let status = child.wait()?;
        assert!(probe?);
        killed?;
        assert_eq!(status.signal(), Some(libc::SIGKILL));
        Ok(())
    }

    #[test]
    #[ignore = "payload selected explicitly by configured_group_can_be_probed_and_killed"]
    fn group_child() {
        loop {
            std::thread::park();
        }
    }
}
