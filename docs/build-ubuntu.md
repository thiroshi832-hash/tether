# Building Tether on Ubuntu (.deb)

Produces `tether-<version>.deb` that installs to `/usr/share/tether`, registers
`tether.service`, and ships the Linux virtual camera (v4l2loopback) + virtual
microphone (PulseAudio null-sink) integration.

Tested target: **Ubuntu 22.04 LTS** (amd64). 24.04 also works with the same
steps; 20.04 works but PipeWire path is untested (falls back to PulseAudio).

---

## 1. System packages

```sh
sudo apt update
sudo apt install -y \
    build-essential clang cmake pkg-config git curl python3 python3-pip \
    nasm yasm ninja-build \
    libgtk-3-dev libxdo-dev libxfixes-dev libxcb1-dev libxcb-randr0-dev \
    libxcb-shape0-dev libxcb-xfixes0-dev libxcb-composite0-dev \
    libasound2-dev libpulse-dev libpam0g-dev libdbus-1-dev \
    libva-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    libssl-dev libclang-dev \
    v4l2loopback-dkms pulseaudio-utils
```

`libclang-dev` is needed by `bindgen` (kcp-sys) — same as the Windows LLVM
requirement. `v4l2loopback-dkms` needs the running kernel's headers; DKMS
usually pulls them in as a dependency.

## 2. Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup default stable
# For flutter_rust_bridge_codegen (nightly cargo-expand):
rustup toolchain install nightly
cargo +stable install --locked flutter_rust_bridge_codegen@1.80.1
cargo +stable install --locked cargo-expand@1.0.95
```

## 3. Flutter

Use Flutter 3.24.5 (same version used on Windows — 3.44+ breaks bridge codegen
against this repo).

```sh
git clone -b 3.24.5 --depth 1 https://github.com/flutter/flutter.git ~/flutter
export PATH="$HOME/flutter/bin:$PATH"
flutter precache --linux
# Apply the dropdown_menu enableFilter patch shipped in the repo:
git -C ~/flutter apply /path/to/tether/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff
```

## 4. Submodule (hbb_common)

`libs/hbb_common` is a submodule and ships empty in this checkout. Same caveat
as the Windows build — the developer's fork carries the custom crypto suite.

```sh
git submodule update --init --recursive
```

If the submodule is empty or its remote isn't set, ask the dev for the fork
URL or drop the `hbb_common` tree in by hand.

## 5. Bridge codegen

```sh
cd tether
flutter_rust_bridge_codegen \
    --rust-input src/flutter_ffi.rs \
    --dart-output flutter/lib/generated_bridge.dart \
    --c-output flutter/macos/Runner/bridge_generated.h
```

(No `--llvm-path` needed on Linux — libclang comes from `libclang-dev`.)

## 6. Build the .deb

From the repo root:

```sh
python3 build.py --flutter
```

Output: `tether-<version>.deb` in the repo root.

Optional flags (mirrors Windows):

- `--hwcodec` — enable hwcodec feature (needs `libva-dev`, already listed).
- `--skip-cargo` — reuse the last `target/release/liblibrustdesk.so` and only
  rebuild the Flutter side + repack.

## 7. Install & smoke-test

```sh
sudo apt install ./tether-<version>.deb
# Systemd starts tether.service. Launch the UI from the desktop entry
# (Applications → Network → Tether) or:
tether &
```

The `.deb` postinst will:

1. `modprobe v4l2loopback` (best-effort) so `/dev/video63` appears.
2. Enable `tether-vmic.service` globally for user sessions — on next login,
   `pactl` creates the "Tether Audio" null-sink + "Tether Microphone"
   remap-source.
3. Enable & start `tether.service`.

### Verify vcam

```sh
v4l2-ctl --list-devices              # expect a "Tether Camera (platform:v4l2loopback-000)" entry
v4l2-ctl -d /dev/video63 --all       # confirm 1280x720 RGB24
```

### Verify vmic

```sh
pactl list short sinks   | grep tether_audio       # null-sink present
pactl list short sources | grep tether_microphone  # remap-source present
```

Zoom / Meet / Chrome should show "Tether Camera" and "Tether Microphone" in
their device pickers.

---

## Common pitfalls

- **DKMS build fails** — `apt install linux-headers-$(uname -r)` and re-run
  `sudo dpkg --configure -a`. Reboot if the kernel just updated.
- **`/dev/video63` missing after install** — `sudo modprobe v4l2loopback`; if
  that fails, `sudo dkms status | grep v4l2loopback` should show `installed`.
- **"Tether Camera" appears but no image** — the operator side's camera has to
  be enabled in the CM window (permission `remote_camera`).
- **"Tether Microphone" missing** — check `systemctl --user status
  tether-vmic.service`; if it's inactive on a headless login, run
  `/usr/share/tether/files/tether-vmic-setup.sh load` in the user session.
- **`flutter build linux` fails on GTK header** — a plugin dep is missing;
  re-check the `apt install` list above.
- **`cargo build` complains about missing `hbb_common::crypto`** — the
  submodule needs the developer's fork (see step 4).
