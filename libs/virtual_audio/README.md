# Tether virtual microphone (virtual audio driver)

Exposes a Windows audio device with:

- a **render** endpoint named **"Tether Audio"** (a speaker), and
- a **capture** endpoint named **"Tether Microphone"** (a mic)

Audio rendered to "Tether Audio" is looped internally to "Tether Microphone".
The Tether app renders the connecting operator's mic audio to "Tether Audio"
(via cpal — see `../../src/virtual_mic.rs` and `AudioHandler` in
`../../src/client.rs`), so any conferencing app on the controlled machine that
selects **"Tether Microphone"** hears the operator.

```
operator mic ──AudioFrame──▶ controlled machine
  controlled: AudioHandler renders to "Tether Audio" (render endpoint)
     driver loopback ─▶ "Tether Microphone" (capture endpoint)
        Zoom/Teams/… select "Tether Microphone" as their mic
```

## Why a kernel driver (and why it must be signed)

Windows has **no** user-mode way to publish a capture (microphone) endpoint that
other apps can select. It requires a PortCls/WaveRT audio driver. And Windows
will not load an **unsigned** audio driver on a normal machine (outside
test-signing mode). So this package **must be attestation-signed** through the
same pipeline used for the IDD virtual-display driver. This is the one
unavoidable manual/CI step — no code change removes it.

## Building the driver (.sys)

Do **not** hand-write a PortCls miniport. Base it on Microsoft's maintained
virtual audio sample, which is already a working render+capture virtual device:

- `Windows-driver-samples/audio/simpleaudiosample` (a.k.a. SYSVAD's simple
  variant). Build with the WDK (matching the Visual Studio + WDK versions used
  for the display driver).

Required modifications (small, localized):

1. **Loopback render → capture.** In the sample's stream handling, copy the
   bytes written to the render (playback) WaveRT stream into the capture
   (recording) stream's buffer (the sample already has both a render and a
   capture circular buffer; wire the render write into the capture read). If you
   start from the SYSVAD "mic array + speaker" topology, connect the speaker
   sink to the mic source.
2. **Endpoint friendly names.** Set the render endpoint name to `Tether Audio`
   and the capture endpoint name to `Tether Microphone` (KSPROPERTY friendly
   name / the `.inf` `AddReg` below). The Rust side matches the render device by
   the substring `Tether Audio` (`RENDER_DEVICE_MATCH`).
3. **Hardware id / service name.** Use hardware id `Root\TetherAudio` and
   service/binary name `tether_audio` so they line up with `tether_audio.inf`
   and the install scripts here.

Output: `tether_audio.sys` + `tether_audio.cat` (signed) + `tether_audio.inf`.
Place all three (plus `install.bat`/`uninstall.bat`) in a folder named
`tether_audio` next to the app executable; `build.py` bundles it.

## Install / uninstall

The app installs on first use (when the operator's mic is first enabled) by
running `install.bat` elevated — see `ensure_installed()` in
`../../src/virtual_mic.rs`. One UAC prompt, no manual steps for the end user.

`install.bat` adds the driver to the driver store and creates the software
(root-enumerated) devnode. It relies on `pnputil` (in-box) and `devgen.exe`
(WDK tool `devgen`); bundle `devgen.exe` in this folder, or adapt to `devcon`.

## Files here

| File | Purpose |
|---|---|
| `tether_audio.inf` | Driver INF: endpoints, friendly names, service |
| `install.bat` | Elevated first-use install (driver store + devnode) |
| `uninstall.bat` | Remove device + driver package |
| `tether_audio.sys` / `.cat` | **Produced by your WDK build + signing** (not in repo) |

## Notes / future work

- One shared virtual device; simultaneous mic-forwarding sessions mix/last-wins.
- Consider muting the local speakers while forwarding, or offering "hear locally
  too" — currently audio goes to the virtual mic only when `remote_mic` is on.
