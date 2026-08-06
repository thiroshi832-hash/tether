//! Windows backend: install/detect the signed kernel-mode virtual audio driver.

use hbb_common::{anyhow::anyhow, bail, log, ResultType};

use super::RENDER_DEVICE_MATCH;

/// Directory (next to the executable) holding the bundled driver package.
const DRIVER_SUBDIR: &str = "tether_audio";
const INF_NAME: &str = "tether_audio.inf";
const INSTALL_SCRIPT: &str = "install.bat";

pub(super) fn is_installed() -> bool {
    use cpal::traits::{DeviceTrait, HostTrait};
    let want = RENDER_DEVICE_MATCH.to_lowercase();
    let host = cpal::default_host();
    if let Ok(devices) = host.output_devices() {
        for d in devices {
            if let Ok(n) = d.name() {
                if n.to_lowercase().contains(&want) {
                    return true;
                }
            }
        }
    }
    false
}

pub(super) fn ensure_installed() -> ResultType<()> {
    if is_installed() {
        return Ok(());
    }
    let dir = driver_dir()?;
    let inf = dir.join(INF_NAME);
    if !inf.exists() {
        bail!("virtual audio driver package not found: {:?}", inf);
    }
    let script = dir.join(INSTALL_SCRIPT);
    log::info!("installing Tether virtual audio driver from {:?}", dir);

    let status = if script.exists() {
        runas::Command::new("cmd")
            .arg("/c")
            .arg(script.to_string_lossy().to_string())
            .status()?
    } else {
        runas::Command::new("pnputil")
            .arg("/add-driver")
            .arg(inf.to_string_lossy().to_string())
            .arg("/install")
            .status()?
    };
    if !status.success() {
        bail!("virtual audio driver install exited with {:?}", status.code());
    }
    // Device appears asynchronously.
    std::thread::sleep(std::time::Duration::from_secs(2));
    Ok(())
}

pub(super) fn uninstall() -> ResultType<()> {
    let dir = driver_dir()?;
    let script = dir.join("uninstall.bat");
    if script.exists() {
        let _ = runas::Command::new("cmd")
            .arg("/c")
            .arg(script.to_string_lossy().to_string())
            .status()?;
    }
    Ok(())
}

fn driver_dir() -> ResultType<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| anyhow!("no exe dir"))?;
    Ok(dir.join(DRIVER_SUBDIR))
}
