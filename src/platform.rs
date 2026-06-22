//! Platform helpers for subprocess spawning.
//!
//! On Windows (GUI-subsystem binary), any child process spawned without
//! CREATE_NO_WINDOW will briefly flash a black console window. Route all
//! short-lived helper spawns through these helpers; intentional UI windows
//! (UAC RunAs prompts, etc.) should bypass them.

/// Apply CREATE_NO_WINDOW to a `std::process::Command` on Windows; no-op elsewhere.
/// Returns a mutable reference to the command so callers can chain further methods.
pub fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Apply CREATE_NO_WINDOW to a `tokio::process::Command` on Windows; no-op elsewhere.
///
/// `tokio::process::Command` exposes `creation_flags` directly on Windows (it
/// re-exports the `std::os::windows::process::CommandExt` impl), so no explicit
/// trait import is required.
pub fn no_window_tokio(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
