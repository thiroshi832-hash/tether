//! Tether virtual camera.
//!
//! The connecting operator's webcam frames arrive on the controlled machine as
//! JPEG (`TetherCameraFrame` -> CM -> [`crate::flutter`]). Instead of only
//! showing them in the CM window, we publish them to an OS virtual camera
//! device named "Tether Camera" that consumer apps (Zoom/Teams/Chrome) see as
//! a normal webcam.
//!
//! Windows: DirectShow source filter reading a shared-memory frame section
//! (see `libs/virtual_camera`). Linux: v4l2loopback `/dev/video63` fed via
//! plain `write(2)` (see `res/tether-vcam.modprobe.conf`).

use hbb_common::{bail, log, ResultType};

/// Target frame size published to the virtual device. Consumer apps negotiate
/// this once at open time. Constants MUST match `libs/virtual_camera/src/shared.h`
/// (Windows filter) and `res/tether-vcam.modprobe.conf` (Linux device caps).
pub const FRAME_W: u32 = 1280;
pub const FRAME_H: u32 = 720;

#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

// Re-export the DirectShow CLSID so callers that need to check registration
// keep working after the module was restructured. Linux has no equivalent.
#[cfg(windows)]
pub use windows::FILTER_CLSID;

#[cfg(windows)]
pub use windows::ensure_registered;

/// Feed one webcam JPEG frame into the virtual camera. Called on the controlled
/// machine from the CM once the `remote_camera` permission is on. Lazily
/// initializes the backend on first use.
pub fn feed_jpeg_frame(_jpeg: &[u8]) {
    #[cfg(any(windows, target_os = "linux"))]
    {
        let rgb = match jpeg_to_rgb_top_down(_jpeg) {
            Ok(b) => b,
            Err(e) => {
                log::debug!("virtual camera decode failed: {}", e);
                return;
            }
        };
        #[cfg(windows)]
        windows::feed(&rgb);
        #[cfg(target_os = "linux")]
        linux::feed(&rgb);
    }
}

/// Tear down the virtual camera (e.g. when the last camera-forwarding
/// connection ends). The device / registered filter stays installed.
#[allow(dead_code)]
pub fn stop() {
    #[cfg(windows)]
    windows::stop();
    #[cfg(target_os = "linux")]
    linux::stop();
}

/// Decode `jpeg`, scale with aspect-preserving letterbox to `FRAME_W x FRAME_H`,
/// return an RGB24 top-down buffer of exactly `FRAME_W * FRAME_H * 3` bytes.
/// Each backend does its own final pixel-format conversion (Windows: BGR
/// bottom-up; Linux: RGB24 as-is).
#[cfg(any(windows, target_os = "linux"))]
fn jpeg_to_rgb_top_down(jpeg: &[u8]) -> ResultType<Vec<u8>> {
    use image::{imageops, GenericImageView, Rgb, RgbImage};
    let img = image::load_from_memory(jpeg)?;
    let (sw, sh) = img.dimensions();
    if sw == 0 || sh == 0 {
        bail!("empty camera frame");
    }
    let scaled = img
        .resize(FRAME_W, FRAME_H, imageops::FilterType::Triangle)
        .to_rgb8();
    let (rw, rh) = (scaled.width(), scaled.height());
    let ox = (FRAME_W - rw) / 2;
    let oy = (FRAME_H - rh) / 2;
    let mut canvas: RgbImage = RgbImage::from_pixel(FRAME_W, FRAME_H, Rgb([0, 0, 0]));
    imageops::replace(&mut canvas, &scaled, ox as i64, oy as i64);
    Ok(canvas.into_raw())
}
