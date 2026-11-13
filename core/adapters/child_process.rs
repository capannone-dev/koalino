//! Waiting for child processes inside the desktop app, where someone else may reap them.
//!
//! The Flutter engine reaps the processes it starts with `waitpid(-1)` for as long as it has any
//! running (`notify-send` while a call offer is shown, for instance). That can collect the exit
//! status of a process the core started too, and the core's own wait then fails with `ECHILD`.
//! The process has ended by then, and its output pipes hold everything it wrote, so callers keep
//! the output and treat the status as unknown.

/// `ECHILD` on Linux and Android.
const NO_CHILD: i32 = 10;

/// Whether a wait failed because the child's status was collected elsewhere.
///
/// Inputs: The error from `Child::wait` or `Child::try_wait`. Outputs: true for `ECHILD` on
/// Linux and Android (Windows has no such reaping). Side effects: None.
pub fn reaped_elsewhere(error: &std::io::Error) -> bool {
    cfg!(any(target_os = "linux", target_os = "android")) && error.raw_os_error() == Some(NO_CHILD)
}

#[cfg(test)]
mod tests {
    //! Checks which wait failures mean "collected elsewhere".

    use super::reaped_elsewhere;

    #[test]
    fn only_echild_on_linux_counts() {
        let echild = std::io::Error::from_raw_os_error(10);
        assert_eq!(reaped_elsewhere(&echild), cfg!(target_os = "linux"));
        assert!(!reaped_elsewhere(&std::io::Error::from_raw_os_error(4)));
        assert!(!reaped_elsewhere(&std::io::Error::other("other")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_child_reaped_by_someone_else_fails_with_echild() {
        use std::process::{Command, Stdio};
        let mut child = Command::new("true")
            .stdout(Stdio::null())
            .spawn()
            .unwrap_or_else(|error| panic!("{error}"));
        let pid = libc_pid(child.id());
        // Plays the Flutter engine's part: collect this child's status first.
        let mut status = 0;
        // SAFETY: waitpid on our own child's ID with a valid status pointer.
        unsafe { waitpid(pid, &mut status, 0) };
        let error = child
            .wait()
            .err()
            .unwrap_or_else(|| panic!("the wait should fail"));
        assert!(reaped_elsewhere(&error), "{error}");
    }

    #[cfg(target_os = "linux")]
    fn libc_pid(id: u32) -> i32 {
        i32::try_from(id).unwrap_or_else(|error| panic!("{error}"))
    }

    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }
}
