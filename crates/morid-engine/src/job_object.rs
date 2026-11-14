//! Windows Job Object ownership and operations; no raw handles in the public API.
#![allow(unsafe_code)]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::Child;
use windows_sys::Win32::Foundation::{ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_LIMIT_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::{
    GetProcessIdOfThread, OpenThread, ResumeThread, CREATE_SUSPENDED, IO_COUNTERS,
    THREAD_QUERY_LIMITED_INFORMATION, THREAD_SUSPEND_RESUME,
};

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

    /// Start without executing backend code until membership is established.
    /// On every setup failure, terminate and reap the still-suspended child.
    pub fn spawn(&self, spec: &crate::spawn::SpawnSpec) -> io::Result<Child> {
        let mut command = spec.command();
        command.creation_flags(CREATE_SUSPENDED);
        let mut child = command.spawn()?;
        let prepared = self
            .assign(&child)
            .and_then(|()| resume_initial_thread(&child));
        if let Err(error) = prepared {
            if let Err(cleanup) = child.kill().and_then(|()| child.wait().map(|_| ())) {
                return Err(io::Error::other(format!(
                    "{error}; suspended-child cleanup: {cleanup}"
                )));
            }
            return Err(error);
        }
        Ok(child)
    }

    /// Assign-after-spawn deliberately has a residual escape window: descendants
    /// already running before this call are not retroactively enrolled. The test
    /// `assign_after_spawn_leaves_preassignment_grandchild_alive` forces that
    /// window and checks the survivor in the Windows process table, then cleans
    /// it up. This API is not a whole-tree cancellation guarantee (criterion 5).
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

    /// Query kernel accounting, not the leader's exit code. A Job has zero
    /// active processes only after all its contained processes have terminated.
    pub fn is_empty(&self) -> io::Result<bool> {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION {
            TotalUserTime: 0,
            TotalKernelTime: 0,
            ThisPeriodTotalUserTime: 0,
            ThisPeriodTotalKernelTime: 0,
            TotalPageFaultCount: 0,
            TotalProcesses: 0,
            ActiveProcesses: 0,
            TotalTerminatedProcesses: 0,
        };
        // SAFETY: the job is uniquely owned and live; the writable buffer is a
        // fully initialized, correctly aligned struct for this information class.
        // The size matches its type; the synchronous call retains no pointers.
        let result = unsafe {
            QueryInformationJobObject(
                self.handle.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                std::ptr::from_mut(&mut accounting).cast(),
                std::mem::size_of_val(&accounting) as u32,
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(accounting.ActiveProcesses == 0)
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

/// The child is still suspended and unreaped; it cannot itself create another
/// thread. Reject ambiguous snapshots rather than resuming an arbitrary thread.
fn resume_initial_thread(child: &Child) -> io::Result<()> {
    // SAFETY: snapshot flags are valid constants; no pointers are passed. The
    // returned snapshot is checked for INVALID_HANDLE_VALUE before ownership.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful snapshot creation transfers a unique closeable handle;
    // this is its sole ownership conversion, and OwnedHandle closes every path.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        cntUsage: 0,
        th32ThreadID: 0,
        th32OwnerProcessID: 0,
        tpBasePri: 0,
        tpDeltaPri: 0,
        dwFlags: 0,
    };
    // SAFETY: snapshot remains live; entry is initialized, aligned and sized
    // for THREADENTRY32. The API writes only synchronously to that struct.
    let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    let mut thread_id = None;
    loop {
        if found == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
                return Err(error);
            }
            break;
        }
        if entry.th32OwnerProcessID == child.id() && thread_id.replace(entry.th32ThreadID).is_some()
        {
            return Err(io::Error::other("suspended child has multiple threads"));
        }
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        // SAFETY: same owned snapshot and initialized entry as above; the API
        // does not retain the entry pointer between calls.
        found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    let id = thread_id.ok_or_else(|| io::Error::other("suspended child thread not found"))?;
    let rights = THREAD_SUSPEND_RESUME | THREAD_QUERY_LIMITED_INFORMATION;
    // SAFETY: id came from a checked snapshot entry, not a guessed process id;
    // no pointers are passed. The resulting handle is checked before ownership.
    let raw = unsafe { OpenThread(rights, 0, id) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenThread returned a unique owned handle; this is its only
    // ownership conversion. The handle is non-inheritable and closed on return.
    let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
    // SAFETY: thread is an owned live handle with query rights. Verify ownership
    // after opening, so a stale snapshot/recycled thread id cannot resume a stranger.
    let owner = unsafe { GetProcessIdOfThread(thread.as_raw_handle()) };
    if owner != child.id() {
        return Err(io::Error::other(
            "snapshot thread no longer belongs to child",
        ));
    }
    // SAFETY: the owned handle pins the verified child's thread; the child was
    // created suspended and assigned to this job before reaching this call.
    let previous = unsafe { ResumeThread(thread.as_raw_handle()) };
    if previous == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    if previous != 1 {
        return Err(io::Error::other("unexpected initial thread suspend count"));
    }
    Ok(())
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

static INTERRUPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "system" fn interrupt_handler(event: u32) -> windows_sys::core::BOOL {
    if event == 0 || event == 1 {
        INTERRUPTED.store(true, std::sync::atomic::Ordering::Relaxed);
        1
    } else {
        0
    }
}
pub fn install_interrupt_handler() -> io::Result<()> {
    INTERRUPTED.store(false, std::sync::atomic::Ordering::Relaxed);
    // SAFETY: function has the Windows console-handler ABI, static lifetime and
    // only stores to an atomic. It never calls console APIs or unwinds.
    if unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(interrupt_handler), 1)
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub fn interrupted() -> bool {
    INTERRUPTED.load(std::sync::atomic::Ordering::Relaxed)
}
