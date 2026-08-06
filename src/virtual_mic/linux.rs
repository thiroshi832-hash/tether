//! Linux backend: PulseAudio (or the pipewire-pulse shim) null-sink acting as
//! the "Tether Audio" render endpoint, with a remap-source "Tether Microphone"
//! taking that sink's monitor as its input.
//!
//! The heavy lifting lives in `res/tether-vmic-setup.sh` (installed at
//! `/usr/share/tether/files/tether-vmic-setup.sh` by the .deb). The per-user
//! systemd unit `tether-vmic.service` runs `load` at login; this module is a
//! fallback for users who don't have systemd --user or who launched Tether
//! before the unit ran.

use std::process::Command;

use hbb_common::{bail, log, ResultType};

use super::RENDER_DEVICE_MATCH;

const SETUP_SCRIPT: &str = "/usr/share/tether/files/tether-vmic-setup.sh";

pub(super) fn is_installed() -> bool {
    // pactl list short sinks: rows are "<id>\t<name>\t<driver>\t...". Match on
    // the sink name (tether_audio) which is stable across setup runs.
    let out = match Command::new("pactl").args(["list", "short", "sinks"]).output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => return false,
    };
    let text = String::from_utf8_lossy(&out);
    text.lines().any(|line| {
        line.split('\t')
            .nth(1)
            .map(|name| name == RENDER_DEVICE_MATCH)
            .unwrap_or(false)
    })
}

pub(super) fn ensure_installed() -> ResultType<()> {
    if is_installed() {
        return Ok(());
    }
    // pactl only works against a user session's PulseAudio/PipeWire — running
    // this from a system service is not useful. Fail soft if the helper isn't
    // present (development builds outside the .deb).
    if !std::path::Path::new(SETUP_SCRIPT).exists() {
        // Fall back to inline pactl commands so an out-of-package build still
        // works. Best-effort — mirror what the script does.
        return inline_setup();
    }
    log::info!("loading Tether virtual mic PulseAudio modules");
    let status = Command::new("sh").arg(SETUP_SCRIPT).arg("load").status()?;
    if !status.success() {
        bail!("tether-vmic-setup.sh load exited with {:?}", status.code());
    }
    Ok(())
}

pub(super) fn uninstall() -> ResultType<()> {
    if std::path::Path::new(SETUP_SCRIPT).exists() {
        let _ = Command::new("sh").arg(SETUP_SCRIPT).arg("unload").status()?;
    }
    Ok(())
}

/// Direct-`pactl` fallback when the packaged helper script isn't on disk.
/// Keep in sync with `res/tether-vmic-setup.sh`.
fn inline_setup() -> ResultType<()> {
    let sink = load_module(&[
        "load-module",
        "module-null-sink",
        "sink_name=tether_audio",
        // Single quotes so pactl's arg parser keeps the space in the name.
        "sink_properties=device.description='Tether Audio'",
    ])?;
    log::info!("Tether virtual mic: null-sink loaded (module id {})", sink);
    let src = load_module(&[
        "load-module",
        "module-remap-source",
        "source_name=tether_microphone",
        "master=tether_audio.monitor",
        "source_properties=device.description='Tether Microphone'",
    ])?;
    log::info!("Tether virtual mic: remap-source loaded (module id {})", src);
    Ok(())
}

fn load_module(args: &[&str]) -> ResultType<String> {
    let out = Command::new("pactl").args(args).output()?;
    if !out.status.success() {
        bail!(
            "pactl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
