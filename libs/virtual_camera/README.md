# Tether virtual camera (DirectShow source filter)

`tether_vcam.dll` is a DirectShow push-source video capture filter that shows up
to other Windows apps (Zoom, Teams, Chrome, Discord, OBS, …) as a webcam named
**Tether Camera**. It serves frames the Tether app publishes into shared memory
(see `src/shared.h` and the Rust writer in `../../src/virtual_camera.rs`).

## How it fits together

```
operator webcam ──JPEG(TetherCameraFrame)──▶ controlled machine
   controlled: src/virtual_camera.rs  ──shared mem (BGR24 1280x720)──▶
   tether_vcam.dll (loaded inside Zoom/Teams/…)  ──▶ "Tether Camera" webcam
```

The Rust side scales/letterboxes each frame to a fixed 1280x720 BGR24 bottom-up
DIB, so the filter's `FillBuffer` is a single `memcpy`. When no new frame is
available the filter repeats the last one (black until the first frame).

## Building

Requires MSVC + the **DirectShow base classes** (`streams.h` / `strmbase`),
which Microsoft ships as *source* in the Windows SDK samples
(`Samples\multimedia\directshow\baseclasses`) or the classic
`Windows-classic-samples` repo. Build them into a static lib, then:

```bat
cmake -S libs/virtual_camera -B build/vcam -A x64 ^
      -DBASECLASSES_DIR=C:\path\to\baseclasses
cmake --build build/vcam --config Release
```

Or, with a prebuilt strmbase (e.g. vcpkg):

```bat
cmake -S libs/virtual_camera -B build/vcam -A x64 ^
      -DSTRMBASE_LIB=... -DSTRMBASE_INCLUDE=...
```

Output: `tether_vcam.dll`. `build.py` copies it next to the main executable.

## Registration

The DLL self-registers as a COM video input device. The Tether app calls
`regsvr32 /s tether_vcam.dll` (elevated) automatically the first time the
operator's camera is enabled — no manual step. To register/unregister by hand:

```bat
regsvr32 tether_vcam.dll        :: install
regsvr32 /u tether_vcam.dll     :: remove
```

## Signing

Being a user-mode COM DLL, this needs only ordinary **Authenticode** signing
(to avoid SmartScreen warnings) — *not* driver signing. Sign `tether_vcam.dll`
with the same code-signing cert used for the app.

## Notes / future work

- Fixed 1280x720@30 output. Multiple simultaneous camera-forwarding sessions
  share one device (last writer wins).
- DirectShow-only: a small number of Win11 Media-Foundation-only apps won't
  enumerate it; a `MFCreateVirtualCamera` path can be added later.
