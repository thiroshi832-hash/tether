# Building Tether on macOS (.dmg)

Produces `Tether.app` (bundle id `com.tether.tether`) and packages it as
`tether-<version>.dmg`. This is an **unsigned / ad-hoc** build — it runs on
your own Macs but needs a Gatekeeper bypass on first launch (see below), and
the macOS **virtual camera / virtual microphone are not included** (they are
system extensions that cannot load without an Apple Developer signature — see
"Deferred" at the end).

Must be built **on a Mac** — Xcode's toolchain can't run on Windows/Linux.

Tested targets: macOS 13/14 on Apple Silicon (arm64) and Intel (x86_64).

---

## 1. Toolchain

```sh
# Xcode + command line tools (from the App Store, then:)
xcode-select --install
sudo xcodebuild -license accept

# Homebrew packages
brew install cmake pkg-config nasm yasm ninja create-dmg cocoapods

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup default stable
# host target: arm64 Macs -> aarch64-apple-darwin, Intel -> x86_64-apple-darwin
# For the OTHER arch (cross-build), add it: rustup target add x86_64-apple-darwin

# flutter_rust_bridge codegen (same versions as the other platforms)
cargo install --locked flutter_rust_bridge_codegen@1.80.1
cargo install --locked cargo-expand@1.0.95
```

Flutter (use 3.24.5 — matches the Windows/Linux builds):

```sh
git clone -b 3.24.5 --depth 1 https://github.com/flutter/flutter.git ~/flutter
export PATH="$HOME/flutter/bin:$PATH"
flutter precache --macos
git -C ~/flutter apply /path/to/tether/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff
```

## 2. Submodule (hbb_common)

Same caveat as the other platforms — `libs/hbb_common` is the developer's fork
(custom crypto suite). It must be populated before building:

```sh
git submodule update --init --recursive
```

## 3. vcpkg (codec dependencies)

scrap/ffmpeg pull codec libs via vcpkg, same as Windows/Linux. Check out vcpkg
at the baseline commit pinned in the repo's `vcpkg.json` (a fresh clone ships
tools too new for the pinned ports):

```sh
git clone https://github.com/microsoft/vcpkg ~/vcpkg
cd ~/vcpkg && git checkout $(grep -o '"baseline": *"[0-9a-f]*"' /path/to/tether/vcpkg.json | grep -o '[0-9a-f]\{40\}')
./bootstrap-vcpkg.sh
export VCPKG_ROOT=~/vcpkg
# arm64 Macs: arm64-osx, Intel: x64-osx
$VCPKG_ROOT/vcpkg install --triplet arm64-osx --x-install-root="$VCPKG_ROOT/installed"
```

(Use `--triplet x64-osx` on Intel.)

## 4. Bridge codegen

```sh
cd /path/to/tether
flutter_rust_bridge_codegen \
    --rust-input src/flutter_ffi.rs \
    --dart-output flutter/lib/generated_bridge.dart \
    --c-output flutter/macos/Runner/bridge_generated.h
cd flutter && flutter pub get && cd ..
```

`flutter pub get` matters — a stale `flutter/.dart_tool` causes mass
`invalid-type … NativeType` FFI errors at build time. Always run it before a build.

## 5. Build

From the repo root:

```sh
export VCPKG_ROOT=~/vcpkg
python3 build.py --flutter
```

This builds the Rust dylib for the host arch, runs `flutter build macos`
(forced to ad-hoc unsigned so it doesn't require the upstream Developer team),
and packages `tether-<version>.dmg` in the repo root. Add `--skip-cargo` to
reuse an existing `target/release/liblibrustdesk.dylib`.

The build is restricted to the host architecture (the Rust dylib is single-arch).
To produce the other arch, build on a machine of that arch, or cross-build the
Rust dylib (`cargo build --target x86_64-apple-darwin …`) and pass
`FLUTTER_XCODE_ARCHS=x86_64`. A universal (lipo'd) binary is out of scope here.

## 6. Run the unsigned app

Because it's ad-hoc signed, Gatekeeper blocks the first launch. Either:

```sh
xattr -dr com.apple.quarantine /Applications/Tether.app
```

or right-click the app → **Open** → **Open** in the dialog.

### Grant permissions (controlled Mac)

macOS gates the capabilities Tether needs behind TCC prompts — grant them under
**System Settings → Privacy & Security**:

- **Screen Recording** — required to capture/share the screen.
- **Accessibility** — required to inject keyboard/mouse input.
- **Microphone** — for audio forwarding (prompted on first use).

Without Screen Recording + Accessibility, a remote controller connects but sees
a black screen and can't control the Mac.

---

## Common pitfalls

- **`invalid-type … NativeType` errors** — stale `flutter/.dart_tool`; run
  `flutter pub get` in `flutter/` and rebuild.
- **Signing error / "requires a development team"** — build.py already forces
  ad-hoc signing; if you invoke `flutter build macos` directly, pass
  `FLUTTER_XCODE_CODE_SIGN_STYLE=Manual FLUTTER_XCODE_DEVELOPMENT_TEAM= FLUTTER_XCODE_CODE_SIGN_IDENTITY=-`.
- **App launches then quits immediately / can't find `AppDelegate`** — the
  MainMenu.xib `customModule` must equal `PRODUCT_NAME` (Tether). This is
  already fixed in the tree; only relevant if you rename the app again.
- **`create-dmg: command not found`** — `brew install create-dmg`. The `.app`
  is still built under `flutter/build/macos/Build/Products/Release/` even
  without it.
- **`liblibrustdesk.dylib` not found by Xcode** — the cargo step didn't run or
  failed; build without `--skip-cargo`, and confirm `VCPKG_ROOT` is exported.
- **`cargo build` complains about missing `hbb_common::crypto`** — the
  submodule needs the developer's fork (step 2).

---

## Deferred (need an Apple Developer account)

These are intentionally **not** part of this build:

- **Code signing + notarization** — required to distribute to other people's
  Macs without the quarantine bypass. Needs a Developer ID cert; then sign the
  app + dylib and `notarytool submit`.
- **Virtual camera / virtual microphone** — on macOS these are a CoreMediaIO
  Camera Extension and a CoreAudio HAL plug-in (system extensions). macOS will
  not load them unless they're signed with the system-extension entitlement and
  notarized. The forwarded webcam still shows in the connection-manager preview;
  it just isn't published as a selectable OS device. Revisit once signing is set
  up.
