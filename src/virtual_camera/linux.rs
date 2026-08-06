//! Linux backend: write RGB24 frames to a v4l2loopback device that consumer
//! apps see as a normal webcam named "Tether Camera".
//!
//! The `.deb` (see `res/tether-vcam.modules-load.conf` +
//! `res/tether-vcam.modprobe.conf`) pins the device to `/dev/video63` with
//! `card_label="Tether Camera"`. We fall back to a scan of `/dev/video*` in
//! case the user manually configured a different `video_nr` — the first
//! device whose `card` field matches wins.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::unix::io::{AsRawFd, RawFd},
    path::PathBuf,
    sync::Mutex,
};

use hbb_common::{bail, log, ResultType};

use super::{FRAME_H, FRAME_W};

const BPP: u32 = 3; // RGB24
const DATA_LEN: usize = (FRAME_W * FRAME_H * BPP) as usize;

/// Preferred v4l2loopback device pinned by `res/tether-vcam.modprobe.conf`.
const PINNED_DEVICE: &str = "/dev/video63";
const CARD_MATCH: &str = "Tether Camera";

// --- V4L2 constants + structs (subset we need) ---
// https://www.kernel.org/doc/html/latest/userspace-api/media/v4l/vidioc-g-fmt.html

const V4L2_BUF_TYPE_VIDEO_OUTPUT: u32 = 2;
const V4L2_FIELD_NONE: u32 = 1;
const V4L2_COLORSPACE_SRGB: u32 = 8;
// V4L2 fourcc for RGB24 packed ("RGB3" little-endian).
const V4L2_PIX_FMT_RGB24: u32 = ((b'R' as u32))
    | ((b'G' as u32) << 8)
    | ((b'B' as u32) << 16)
    | ((b'3' as u32) << 24);

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct V4l2PixFormat {
    width: u32,
    height: u32,
    pixelformat: u32,
    field: u32,
    bytesperline: u32,
    sizeimage: u32,
    colorspace: u32,
    priv_: u32,
    flags: u32,
    // union { ycbcr_enc, hsv_enc } - we always leave zero (driver picks default).
    ycbcr_enc: u32,
    quantization: u32,
    xfer_func: u32,
}

/// `struct v4l2_format` = `__u32 type; union { ... u8[200] } fmt;`. We only
/// need the pix variant; pad the rest of the union so `sizeof` matches what
/// the kernel expects for the ioctl.
#[repr(C)]
struct V4l2Format {
    type_: u32,
    fmt_pix: V4l2PixFormat,
    // Union in the kernel struct is 200 bytes; V4l2PixFormat is 48 bytes
    // (12 x u32). Hardcoded to avoid depending on `const size_of` MSRV.
    _padding: [u8; 152],
}

impl V4l2Format {
    fn new_output_rgb24() -> Self {
        Self {
            type_: V4L2_BUF_TYPE_VIDEO_OUTPUT,
            fmt_pix: V4l2PixFormat {
                width: FRAME_W,
                height: FRAME_H,
                pixelformat: V4L2_PIX_FMT_RGB24,
                field: V4L2_FIELD_NONE,
                bytesperline: FRAME_W * BPP,
                sizeimage: DATA_LEN as u32,
                colorspace: V4L2_COLORSPACE_SRGB,
                ..Default::default()
            },
            _padding: [0u8; 152],
        }
    }
}

// `struct v4l2_capability` — used by VIDIOC_QUERYCAP so we can locate the
// tether device by card_label when the pinned path is unavailable.
#[repr(C)]
#[derive(Copy, Clone)]
struct V4l2Capability {
    driver: [u8; 16],
    card: [u8; 32],
    bus_info: [u8; 32],
    version: u32,
    capabilities: u32,
    device_caps: u32,
    reserved: [u32; 3],
}

nix::ioctl_readwrite!(vidioc_s_fmt, b'V', 5, V4l2Format);
nix::ioctl_read!(vidioc_querycap, b'V', 0, V4l2Capability);

// --- state ---

struct Backend {
    file: File,
}

impl Backend {
    fn open() -> ResultType<Self> {
        let path = locate_device()?;
        let file = OpenOptions::new().read(true).write(true).open(&path)?;
        set_format(file.as_raw_fd())?;
        log::info!("Tether virtual camera opened: {:?}", path);
        Ok(Self { file })
    }

    fn write(&mut self, rgb: &[u8]) {
        if rgb.len() != DATA_LEN {
            log::error!(
                "virtual camera frame size mismatch: {} != {}",
                rgb.len(),
                DATA_LEN
            );
            return;
        }
        // Blocking write to /dev/videoN. v4l2loopback discards frames when no
        // consumer is attached, so this stays fast under back-pressure.
        if let Err(e) = self.file.write_all(rgb) {
            log::debug!("virtual camera write failed: {}", e);
        }
    }
}

fn set_format(fd: RawFd) -> ResultType<()> {
    let mut fmt = V4l2Format::new_output_rgb24();
    // SAFETY: fd is a valid v4l2 device we just opened; fmt is stack-owned and
    // outlives the ioctl call.
    let rc = unsafe { vidioc_s_fmt(fd, &mut fmt as *mut _) };
    match rc {
        Ok(_) => {
            // v4l2loopback may snap dimensions to what it was configured with —
            // log the accepted values so mismatch is obvious in the log.
            log::info!(
                "Tether vcam VIDIOC_S_FMT: {}x{} pixfmt=0x{:08x}",
                fmt.fmt_pix.width,
                fmt.fmt_pix.height,
                fmt.fmt_pix.pixelformat,
            );
            Ok(())
        }
        Err(e) => bail!("VIDIOC_S_FMT failed: {}", e),
    }
}

/// Prefer the pinned `/dev/video63`. Otherwise scan `/dev/video*` and match on
/// `card` = "Tether Camera".
fn locate_device() -> ResultType<PathBuf> {
    let pinned = PathBuf::from(PINNED_DEVICE);
    if pinned.exists() {
        return Ok(pinned);
    }
    let entries = std::fs::read_dir("/dev")?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else { continue };
        if !name_str.starts_with("video") {
            continue;
        }
        let path = entry.path();
        if let Ok(f) = OpenOptions::new().read(true).write(true).open(&path) {
            if card_matches(f.as_raw_fd(), CARD_MATCH) {
                return Ok(path);
            }
        }
    }
    bail!(
        "no v4l2loopback device named '{}' found; is v4l2loopback loaded? \
         (see /etc/modprobe.d/tether-vcam.conf)",
        CARD_MATCH
    );
}

fn card_matches(fd: RawFd, want: &str) -> bool {
    let mut cap = V4l2Capability {
        driver: [0; 16],
        card: [0; 32],
        bus_info: [0; 32],
        version: 0,
        capabilities: 0,
        device_caps: 0,
        reserved: [0; 3],
    };
    // SAFETY: fd is a valid device just opened; cap is stack-owned.
    if unsafe { vidioc_querycap(fd, &mut cap as *mut _) }.is_err() {
        return false;
    }
    let end = cap.card.iter().position(|&b| b == 0).unwrap_or(cap.card.len());
    std::str::from_utf8(&cap.card[..end])
        .map(|s| s.contains(want))
        .unwrap_or(false)
}

lazy_static::lazy_static! {
    static ref INSTANCE: Mutex<Option<Backend>> = Mutex::new(None);
}

pub(super) fn feed(rgb: &[u8]) {
    let mut guard = INSTANCE.lock().unwrap();
    if guard.is_none() {
        match Backend::open() {
            Ok(b) => *guard = Some(b),
            Err(e) => {
                log::warn!("Tether virtual camera unavailable: {}", e);
                return;
            }
        }
    }
    if let Some(b) = guard.as_mut() {
        b.write(rgb);
    }
}

pub(super) fn stop() {
    *INSTANCE.lock().unwrap() = None;
}
