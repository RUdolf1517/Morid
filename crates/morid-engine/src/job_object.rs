//! Windows Job Object ownership and operations; no raw handles in the public API.
#![allow(unsafe_code)]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::Child;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_BASIC_LIMIT_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::IO_COUNTERS;

/// A non-inheritable, uniquely owned job. OwnedHandle closes it safely on every
/// error path and on drop; KILL_ON_JOB_CLOSE applies to its process members.
pub struct Job {
    handle: OwnedHandle,
}

impl Job {
    pub fn new() -> io::Result<Self> {
        // SAFETY: null attributes request a non-inheritable handle; a null name
        // creates a fresh unnamed job. No caller-owned memory is passed.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: CreateJobObjectW just returned a non-null owned handle. This
        // is its sole ownership conversion; no other owner closes or duplicates it.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        // Explicit field initializers avoid an otherwise unnecessary zeroed block.
        let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                PerProcessUserTimeLimit: 0,
                PerJobUserTimeLimit: 0,
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                MinimumWorkingSetSize: 0,
                MaximumWorkingSetSize: 0,
                ActiveProcessLimit: 0,
                Affinity: 0,
                PriorityClass: 0,
                SchedulingClass: 0,
            },
            IoInfo: IO_COUNTERS {
                ReadOperationCount: 0,
                WriteOperationCount: 0,
                OtherOperationCount: 0,
                ReadTransferCount: 0,
                WriteTransferCount: 0,
                OtherTransferCount: 0,
            },
            ProcessMemoryLimit: 0,
            JobMemoryLimit: 0,
            PeakProcessMemoryUsed: 0,
            PeakJobMemoryUsed: 0,
        };
        // SAFETY: handle remains owned/live; limits is fully initialized and
        // correctly aligned. Its type/size match this information class, and the
        // API copies the buffer synchronously without retaining the pointer.
        let configured = unsafe {
            SetInformationJobObject(
                handle.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle })
    }

    /// Primitive only: this does not make spawn-then-assign race-free. A future
    /// supervisor must establish membership before backend code can spawn children.
    /// It must not claim containment merely because this call succeeded later.
    pub fn assign(&self, child: &Child) -> io::Result<()> {
        let job = self.handle.as_raw_handle();
        let process = child.as_raw_handle();
        // SAFETY: both job and process handles are borrowed from live Rust owners
        // for this call. The API does not take ownership of either handle.
        let assigned = unsafe { AssignProcessToJobObject(job, process) };
        if assigned == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Request termination. Completion and descendant absence must be checked
    /// independently by the supervisor, not inferred from this return value.
    pub fn terminate(&self, exit_code: u32) -> io::Result<()> {
        // SAFETY: the handle is an owned live job handle; the integer exit code
        // is opaque to the OS. No pointers to caller-owned memory are passed.
        let terminated = unsafe { TerminateJobObject(self.handle.as_raw_handle(), exit_code) };
        if terminated == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_job_can_be_configured_and_terminated() -> io::Result<()> {
        let job = Job::new()?;
        job.terminate(1)
    }
}
