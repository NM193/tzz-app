# First install on someone else's Mac

**Status:** built 2026-10-23, ships in 0.1.3.

## What was found

The published 0.1.2 disk image was taken apart on 2026-10-23:

- **The bundle carries only the linker's signature.** It covers the binary,
  not the bundle (`Sealed Resources=none`), so `codesign --verify --deep
  --strict` fails with "code has no resources but signature indicates they must
  be present". A copy marked as downloaded is rejected by `spctl` for that
  broken signature. That is the case macOS reports as "damaged", and
  Privacy & Security offers no Open Anyway for it. The same copy signed ad hoc
  is rejected only as unverified, which is the case Open Anyway exists for.
- **Two tools ask for macOS 26**, the version of the Mac that built them:
  `whisper-cli` (cmake, no deployment target) and the OCR helper (`swiftc`, no
  `-target`). On anything older, Whisper and screen text fail. The rest asks
  for 11 or 12.
- **README.md** still describes "YT MP3", built from source with Homebrew's
  tools, and says nothing about the download or the first open.

## Decided

Sign ad hoc and set one minimum macOS, 13.3. Apple's Developer ID and
notarization ($99 a year) would remove the warning altogether; not now, and
nothing here stands in its way later.

### Signing

`tauri.conf.json > bundle > macOS`:

- `"signingIdentity": "-"` signs the whole bundle ad hoc, for every build:
  `npm run build`, `install:app` and a release alike. Tauri's documentation
  gives exactly this case as the reason for it.
- `"hardenedRuntime": false`. Tauri turns it on whenever it signs. It buys
  nothing without notarization, and with it the microphone needs an
  entitlement the app does not have, for itself and for the ffmpeg it records
  through.

macOS will likely ask for the microphone again once, because the signature
that permission was given to changes.

### One minimum macOS

`"minimumSystemVersion": "13.3"`, in the same place, is the only copy of the
number:

- Tauri writes it into `LSMinimumSystemVersion`, so macOS 12 gets a clear
  refusal at launch instead of a failure in the middle of a job, and it sets
  `MACOSX_DEPLOYMENT_TARGET` for the Rust build (only in `tauri build`; dev is
  left alone).
- `build.rs` hands that variable to `swiftc` as `-target`, and asks cargo to
  rebuild the helper when it changes. Without the variable, `cargo test` and
  `tauri dev`, nothing changes.
- `scripts/fetch-tools.sh` reads the number from `tauri.conf.json`, gives it to
  cmake as `CMAKE_OSX_DEPLOYMENT_TARGET`, and counts a tool that asks for a
  newer macOS as missing, so the whisper-cli already on disk is rebuilt on the
  next run without `--force`. It ends by checking all three tools against the
  number.

Why 13.3: ffmpeg asks for 12, the OCR helper uses nothing newer than 10.15,
and the Metal code in whisper.cpp guards what is newer, but ggml's BLAS
backend calls Accelerate's `cblas_sgemm` unguarded, and the interface it uses
arrived in 13.3 (the first build for 13.0 warned about exactly that). The
stylesheet's `color-mix` needs Safari 16.2, which 13.1 ships. Every Apple
Silicon Mac can run 13.3, and any Ventura that took updates already does.

The same build showed one more way to fail on someone else's Mac: ggml
builds its CPU kernels for the Mac it is built on unless told otherwise
(`GGML_NATIVE`). Built on this M5 Pro, whisper-cli held 284 `smmla`
instructions (i8mm), which an M1 does not have. `-DGGML_NATIVE=OFF` builds
for the Apple Silicon baseline instead; the heavy work is on Metal anyway.

### release.sh proves it

After the build and before anything is committed or pushed: the bundle's
signature has to verify with `--deep --strict`, and every binary in
`Contents/MacOS` has to ask for no newer macOS than the minimum. Both were real
bugs, and the script already proves the others rather than assuming them.

### README.md

Written again, for two readers. Someone who wants the app: what it does, where
to download it, Apple Silicon and macOS 13.3 or later, the first open (Open
Anyway on macOS 15 and later, Control-click and Open before that), the Whisper
model from Settings, BlackHole for the Mac's own sound, and that it updates
itself. Someone who builds it: what is needed, the commands, and that
releasing is described in CLAUDE.md.

### Recorded

- CHANGELOG.md, under Unreleased, Fixed: the app opens on another Mac, and
  Whisper and screen text work on any Apple Silicon Mac with 13.3 or later.
- CLAUDE.md: the ad hoc signature and the single minimum, with why.
- The project memory's note about the 0.1.2 image: fixed on main.

## Checking it

- `cargo test` still builds and passes, which also runs the changed `build.rs`
  without the variable.
- `bash scripts/fetch-tools.sh` rebuilds whisper-cli; `vtool` shows it at
  13.3, and its disassembly has no i8mm, bf16 or SME instructions.
- A full `npm run build`, then, on the result: `codesign --verify --deep
  --strict` passes; the signature is ad hoc, without the runtime flag, with
  sealed resources; `Info.plist` says 13.3; every binary in `Contents/MacOS`
  and the OCR helper ask for 13.3 or less; the bundled tools still run; and
  `spctl` on a copy marked as downloaded rejects it without the broken
  signature error. The disk image's copy checks the same.
- `release.sh` is not run. Its new check is pulled out and run against the
  build.
- The owner tries one recording and one Whisper run in the new build before
  the release. There is no older Mac here, so what runs on 13 is known from
  what the binaries ask for, not from running them there.
