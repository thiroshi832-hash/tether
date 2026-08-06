//! Windows backend: publish frames into a named shared-memory section that a
//! DirectShow source filter (`libs/virtual_camera/tether_vcam.dll`) reads.

use std::sync::Mutex;

use hbb_common::{bail, log, ResultType};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0},
        System::{
            Memory::{
                CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
                MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
            },
            Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject, INFINITE},
        },
    },
};

use super::{FRAME_H, FRAME_W};

// Must match libs/virtual_camera/src/shared.h.
const BPP: u32 = 3; // BGR24
const DATA_LEN: usize = (FRAME_W * FRAME_H * BPP) as usize;
const HEADER_LEN: usize = 8 * 4; // 8 x u32
const MAP_LEN: usize = HEADER_LEN + DATA_LEN;
const MAGIC: u32 = 0x3143_5654; // "TVC1"
const VERSION: u32 = 1;
const FORMAT_BGR24_BOTTOMUP: u32 = 0;

const MAP_NAME: PCWSTR = w!("Local\\TetherVirtualCameraFrame");
const MUTEX_NAME: PCWSTR = w!("Local\\TetherVirtualCameraMutex");

/// The DShow filter's CLSID. Keep identical to shared.h and the .rc/registration.
pub const FILTER_CLSID: &str = "{6B1E2C9A-7F43-4E8B-9C2D-1A5E4F0D8B37}";

lazy_static::lazy_static! {
    static ref INSTANCE: Mutex<Option<SharedFrame>> = Mutex::new(None);
}

/// Owns the named file mapping + mutex. Raw pointer stored as `usize` so the
/// struct is `Send` (guarded by the process-wide `INSTANCE` mutex and the
/// cross-process named mutex).
struct SharedFrame {
    map: HANDLE,
    mutex: HANDLE,
    view: usize,
    seq: u32,
}

// SAFETY: `view` only dereferenced while holding both `INSTANCE` (this process)
// and the named mutex (across processes).
unsafe impl Send for SharedFrame {}

impl SharedFrame {
    fn create() -> ResultType<Self> {
        unsafe {
            let map = CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                MAP_LEN as u32,
                MAP_NAME,
            )?;
            let view: MEMORY_MAPPED_VIEW_ADDRESS =
                MapViewOfFile(map, FILE_MAP_ALL_ACCESS, 0, 0, MAP_LEN);
            if view.Value.is_null() {
                let _ = CloseHandle(map);
                bail!("MapViewOfFile failed for virtual camera");
            }
            let mutex = CreateMutexW(None, false, MUTEX_NAME)?;

            let base = view.Value as *mut u32;
            // Initialize the header. sequence stays 0 until the first frame.
            base.add(0).write_volatile(MAGIC);
            base.add(1).write_volatile(VERSION);
            base.add(2).write_volatile(FRAME_W);
            base.add(3).write_volatile(FRAME_H);
            base.add(4).write_volatile(FORMAT_BGR24_BOTTOMUP);
            base.add(5).write_volatile(0); // sequence
            base.add(6).write_volatile(DATA_LEN as u32);
            base.add(7).write_volatile(0); // reserved
            Ok(Self {
                map,
                mutex,
                view: view.Value as usize,
                seq: 0,
            })
        }
    }

    /// Write one fully-prepared BGR24 bottom-up frame (`DATA_LEN` bytes).
    fn write(&mut self, bgr_bottom_up: &[u8]) {
        if bgr_bottom_up.len() != DATA_LEN {
            log::error!(
                "virtual camera frame size mismatch: {} != {}",
                bgr_bottom_up.len(),
                DATA_LEN
            );
            return;
        }
        unsafe {
            if WaitForSingleObject(self.mutex, INFINITE) != WAIT_OBJECT_0 {
                return;
            }
            let base = self.view as *mut u8;
            let data = base.add(HEADER_LEN);
            std::ptr::copy_nonoverlapping(bgr_bottom_up.as_ptr(), data, DATA_LEN);
            self.seq = self.seq.wrapping_add(1);
            // Publish sequence last so a reader never sees a torn frame.
            (base as *mut u32).add(5).write_volatile(self.seq);
            let _ = ReleaseMutex(self.mutex);
        }
    }
}

impl Drop for SharedFrame {
    fn drop(&mut self) {
        unsafe {
            if self.view != 0 {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.view as *mut _,
                });
            }
            let _ = CloseHandle(self.mutex);
            let _ = CloseHandle(self.map);
        }
    }
}

/// Convert RGB24 top-down (as produced by `super::jpeg_to_rgb_top_down`) to the
/// BGR24 bottom-up DIB the filter's `FillBuffer` expects.
fn rgb_top_down_to_bgr_bottom_up(rgb: &[u8]) -> Vec<u8> {
    let stride = (FRAME_W * BPP) as usize;
    let mut out = vec![0u8; DATA_LEN];
    for y in 0..FRAME_H as usize {
        let src_row = &rgb[y * stride..(y + 1) * stride];
        let dst_y = FRAME_H as usize - 1 - y;
        let dst_row = &mut out[dst_y * stride..(dst_y + 1) * stride];
        for (s, d) in src_row.chunks_exact(3).zip(dst_row.chunks_exact_mut(3)) {
            d[0] = s[2]; // B
            d[1] = s[1]; // G
            d[2] = s[0]; // R
        }
    }
    out
}

/// Push one already-decoded RGB24 top-down frame into the shared section.
pub(super) fn feed(rgb: &[u8]) {
    let bgr = rgb_top_down_to_bgr_bottom_up(rgb);
    let mut guard = INSTANCE.lock().unwrap();
    if guard.is_none() {
        // Best-effort: make sure the DShow filter is registered so consumer
        // apps can actually see the device. Non-fatal if it fails.
        if let Err(e) = ensure_registered() {
            log::warn!("virtual camera register failed: {}", e);
        }
        match SharedFrame::create() {
            Ok(s) => *guard = Some(s),
            Err(e) => {
                log::error!("virtual camera init failed: {}", e);
                return;
            }
        }
    }
    if let Some(s) = guard.as_mut() {
        s.write(&bgr);
    }
}

pub(super) fn stop() {
    *INSTANCE.lock().unwrap() = None;
}

/// Ensure the DirectShow filter DLL is COM-registered. Registration writes to
/// HKLM, so this elevates via `regsvr32` (one UAC prompt on first use, matching
/// the IDD display-driver install flow). No-op if already registered.
pub fn ensure_registered() -> ResultType<()> {
    if is_registered() {
        return Ok(());
    }
    let dll = filter_dll_path()?;
    if !dll.exists() {
        bail!("tether_vcam.dll not found next to the executable: {:?}", dll);
    }
    log::info!("registering Tether virtual camera filter: {:?}", dll);
    let status = runas::Command::new("regsvr32")
        .arg("/s")
        .arg(dll.to_string_lossy().to_string())
        .status()?;
    if !status.success() {
        bail!("regsvr32 exited with {:?}", status.code());
    }
    Ok(())
}

fn is_registered() -> bool {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    hkcr.open_subkey(format!("CLSID\\{}", FILTER_CLSID)).is_ok()
}

fn filter_dll_path() -> ResultType<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| hbb_common::anyhow::anyhow!("no exe dir"))?;
    Ok(dir.join("tether_vcam.dll"))
}
