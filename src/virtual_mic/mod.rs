//! Tether virtual microphone.
//!
//! The operator's microphone audio arrives on the controlled machine as
//! `AudioFrame`s and is decoded by [`crate::client::AudioHandler`]. Instead of
//! playing to the local speakers, we route playback to a virtual "render"
//! endpoint whose "capture" side conferencing apps select as a mic.
//!
//! Windows: signed kernel driver ("Tether Audio" render endpoint loops to a
//! "Tether Microphone" capture endpoint). Linux: PulseAudio/PipeWire null-sink
//! + remap-source, set up by `res/tether-vmic-setup.sh` (invoked from the per-
//! user systemd service `tether-vmic.service` or on-demand from this module).

use hbb_common::ResultType;

#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

/// Substring matched against output-device names to find the virtual render
/// endpoint. On Windows this hits the .inf-registered "Tether Audio" endpoint;
/// on Linux, the PulseAudio sink named `tether_audio` (see
/// `res/tether-vmic-setup.sh`).
#[cfg(windows)]
pub const RENDER_DEVICE_MATCH: &str = "Tether Audio";
#[cfg(target_os = "linux")]
pub const RENDER_DEVICE_MATCH: &str = "tether_audio";

/// True if the virtual audio render endpoint is currently present.
#[allow(dead_code)]
pub fn is_installed() -> bool {
    #[cfg(windows)]
    {
        return windows::is_installed();
    }
    #[cfg(target_os = "linux")]
    {
        return linux::is_installed();
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        false
    }
}

/// Ensure the virtual audio backend is available. On Windows this installs
/// the kernel driver on first use (one UAC prompt). On Linux this loads the
/// null-sink + remap-source via `pactl`; the persistent systemd user service
/// normally does this at login, so this is a fallback for headless / manual
/// launches.
pub fn ensure_installed() -> ResultType<()> {
    #[cfg(windows)]
    {
        windows::ensure_installed()
    }
    #[cfg(target_os = "linux")]
    {
        linux::ensure_installed()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Ok(())
    }
}

/// Best-effort uninstall (app uninstall). No-op on unsupported platforms.
#[allow(dead_code)]
pub fn uninstall() -> ResultType<()> {
    #[cfg(windows)]
    {
        windows::uninstall()
    }
    #[cfg(target_os = "linux")]
    {
        linux::uninstall()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Ok(())
    }
}
