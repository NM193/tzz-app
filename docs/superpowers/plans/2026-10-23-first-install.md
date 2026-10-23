# First Install Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A copy downloaded from GitHub opens on another Apple Silicon Mac with macOS 13 or later, through Open Anyway rather than "damaged", and Whisper and screen text work there.

**Architecture:** `tauri.conf.json > bundle > macOS` gains an ad hoc signing identity, hardened runtime off, and a minimum macOS of 13.0, the only copy of that number. `build.rs` and `scripts/fetch-tools.sh` pass the number to `swiftc` and cmake, which do not read it on their own. `release.sh` checks the signature and every binary against it before publishing. README.md is rewritten.

**Tech Stack:** Tauri 2 bundler (CLI 2.11.4), Rust build script, bash, cmake 4.4, swiftc 6.3, `codesign` / `spctl` / `vtool`.

**Spec:** `docs/superpowers/specs/2026-10-23-first-install-design.md`

**Changed while executing:** the minimum became 13.3, not 13.0 (ggml's BLAS
backend calls `cblas_sgemm`, which needs 13.3), and whisper is built with
`-DGGML_NATIVE=OFF` (built natively on an M5 it used i8mm instructions an M1
lacks). Where the steps below say 13.0, read 13.3.

**Ground rules:**
- Do not run `scripts/release.sh`: it publishes.
- Never read `~/.tauri/tzz.key`. A local build therefore turns off the updater artifacts, which need it, with `--config` (Task 5).
- Before building, check that no job is running (`pgrep -fl 'whisper-cli|yt-dlp|ffmpeg'`): CLAUDE.md, "Do not edit Rust while a job is running".
- Commit on `main` after each task, never push. Messages are plain sentences and end with the `Co-Authored-By` line shown.

---

## File structure

| File | Change | Responsibility |
|---|---|---|
| `src-tauri/tauri.conf.json` | modify | `bundle.macOS`: ad hoc identity, hardened runtime off, minimum 13.0 |
| `src-tauri/build.rs` | modify | Pass `MACOSX_DEPLOYMENT_TARGET` to `swiftc` as `-target`; rebuild when it changes |
| `scripts/fetch-tools.sh` | modify | Read the minimum; build whisper for it; a too-new tool counts as missing; check all three |
| `scripts/release.sh` | modify | Before committing: the bundle signature verifies, every binary runs on the minimum |
| `README.md` | rewrite | For someone who downloads the app, and for someone who builds it |
| `CHANGELOG.md` | modify | Two lines under Unreleased > Fixed |
| `CLAUDE.md` | modify | The ad hoc signature and the single minimum, with why |

---

### Task 1: Sign ad hoc, and say which macOS

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add `bundle.macOS`**

`release.sh` already rewrites this file with `json.dump(..., indent=2)`, and a round trip leaves it byte for byte the same, so the same tool edits it. JSON has no comments; the reasons go into CLAUDE.md in Task 7.

```bash
python3 - <<'PY'
import collections, json
path = "src-tauri/tauri.conf.json"
conf = json.load(open(path), object_pairs_hook=collections.OrderedDict)
conf["bundle"]["macOS"] = collections.OrderedDict([
    ("signingIdentity", "-"),
    ("hardenedRuntime", False),
    ("minimumSystemVersion", "13.0"),
])
json.dump(conf, open(path, "w"), indent=2)
open(path, "a").write("\n")
PY
git diff src-tauri/tauri.conf.json
```

Expected: the only change is a `"macOS": { "signingIdentity": "-", "hardenedRuntime": false, "minimumSystemVersion": "13.0" }` block added at the end of `"bundle"`, after `"createUpdaterArtifacts": true` (which gains a trailing comma).

- [ ] **Step 2: The changelog line for the signature**

In `CHANGELOG.md`, under `## Unreleased`, replace the first:

```markdown
### Fixed

```

with:

```markdown
### Fixed
- The app opens on a Mac it was not built on. macOS asks once, in System
  Settings under Privacy & Security, instead of calling the app damaged.

```

Run: `python3 scripts/changelog.py text Unreleased`
Expected:
```
Improved
- The update notice says what is new, what improved and what was fixed, with every version since yours when you skipped one.

Fixed
- The app opens on a Mac it was not built on. macOS asks once, in System Settings under Privacy & Security, instead of calling the app damaged.
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json CHANGELOG.md
git commit -m "$(cat <<'EOF'
Sign the whole bundle, and say which macOS it needs

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: The OCR helper is built for the minimum

**Files:**
- Modify: `src-tauri/build.rs:14-32`

- [ ] **Step 1: Show the flag does what is wanted, outside the build**

```bash
S="/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/ocr-target"
mkdir -p "$S"
swiftc -O -o "$S/ocr" -target arm64-apple-macos13.0 src-tauri/helpers/ocr.swift -framework Vision -framework AppKit
vtool -arch arm64 -show-build "$S/ocr" | awk '/minos/ {print "minos", $2}'
```

Expected: no errors or availability warnings, then `minos 13.0`.

- [ ] **Step 2: Pass it from `build.rs`**

Replace:

```rust
fn build_ocr_helper() {
    println!("cargo:rerun-if-changed=helpers/ocr.swift");

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("ocr");

    let status = Command::new("swiftc")
        .args(["-O", "-o"])
        .arg(&out)
        .arg("helpers/ocr.swift")
        .args(["-framework", "Vision", "-framework", "AppKit"])
        .status();
```

with:

```rust
fn build_ocr_helper() {
    println!("cargo:rerun-if-changed=helpers/ocr.swift");
    // `tauri build` sets this from bundle.macOS.minimumSystemVersion, and the
    // Rust is built for it. swiftc does not read it: left alone, the helper
    // asks for the macOS of the Mac that built it, and screen text fails on
    // anything older. Unset -- `tauri dev`, `cargo test` -- nothing changes.
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("ocr");

    let mut swiftc = Command::new("swiftc");
    swiftc
        .args(["-O", "-o"])
        .arg(&out)
        .arg("helpers/ocr.swift")
        .args(["-framework", "Vision", "-framework", "AppKit"]);
    if let Ok(minimum) = std::env::var("MACOSX_DEPLOYMENT_TARGET") {
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        let arch = if arch == "aarch64" { "arm64" } else { arch.as_str() };
        swiftc.arg("-target").arg(format!("{arch}-apple-macos{minimum}"));
    }
    let status = swiftc.status();
```

The `match status { ... }` below it stays as it is.

- [ ] **Step 3: Still builds, and the tests still pass**

Run: `cd src-tauri && cargo test 2>&1 | grep -E '^test result|error' ; cd ..`
Expected: `test result: ok. 97 passed; 0 failed` (and the empty results of the other targets), no `error`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/build.rs
git commit -m "$(cat <<'EOF'
Build the OCR helper for the oldest macOS the app supports

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: whisper-cli is built for the minimum, and a stale one is rebuilt

**Files:**
- Modify: `scripts/fetch-tools.sh`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: The header says what "already here" now means**

Replace:

```bash
# A tool that is already here and answers for itself is left alone, because a
# release runs this every time and 90 MB a release is a toll for nothing.
```

with:

```bash
# A tool that is already here, answers for itself and runs on the oldest macOS
# the app supports is left alone, because a release runs this every time and
# 90 MB a release is a toll for nothing.
```

- [ ] **Step 2: The minimum, and how a binary is measured against it**

Replace:

```bash
mkdir -p "$OUT" "$WORK"
say() { printf '\n== %s\n' "$1"; }
```

with:

```bash
mkdir -p "$OUT" "$WORK"
say() { printf '\n== %s\n' "$1"; }

# The oldest macOS the app supports, from the one place it is set. Every tool
# has to run there. Built here without it, a tool asks for this Mac's macOS:
# whisper-cli once asked for 26 and failed on everything older.
MIN="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["bundle"]["macOS"]["minimumSystemVersion"])')"

# The macOS a binary asks for, as the loader reads it.
minos() { vtool -arch arm64 -show-build "$1" 2> /dev/null | awk '/minos/ {print $2; exit}'; }

# Whether a binary runs on MIN. One that cannot be read does not.
runs_on_min() {
  python3 -c 'import sys; a, b = (tuple(map(int, v.split("."))) for v in sys.argv[1:]); sys.exit(a > b)' \
    "$(minos "$1")" "$MIN" 2> /dev/null
}
```

- [ ] **Step 3: A tool built for a newer macOS is not here**

Replace:

```bash
  [ -x "$tool" ] || return 1
  shift
```

with:

```bash
  [ -x "$tool" ] || return 1
  runs_on_min "$tool" || return 1
  shift
```

- [ ] **Step 4: cmake builds for the minimum**

Replace:

```bash
    -DCMAKE_BUILD_TYPE=Release \
```

with:

```bash
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_DEPLOYMENT_TARGET="$MIN" \
```

and add to the comment above `build_whisper()`, after the `OPENMP=OFF` line:

```bash
#   DEPLOYMENT_TARGET       the oldest macOS the app supports, not this Mac's.
```

- [ ] **Step 5: All three are checked at the end**

Replace:

```bash
say "done"
```

with:

```bash
say "macOS $MIN"
for name in yt-dlp ffmpeg whisper-cli; do
  if ! runs_on_min "$OUT/$name-$TARGET"; then
    echo "   $name asks for macOS $(minos "$OUT/$name-$TARGET"), newer than the app's $MIN"
    exit 1
  fi
done
echo "   every tool runs there"

say "done"
```

- [ ] **Step 6: Syntax, then a real run**

Run: `bash -n scripts/fetch-tools.sh && echo "syntax ok"`
Expected: `syntax ok`

Check no job is running, then run: `bash scripts/fetch-tools.sh`
Expected: `yt-dlp` and `ffmpeg` say `already here` (they ask for 11.0 and 12.0); `whisper-cli` is rebuilt, because the one on disk asks for 26.0, and ends with `self-contained`; then `== macOS 13.0` and `every tool runs there`. The rebuild pulls whisper.cpp and takes a few minutes.

Run: `for f in src-tauri/binaries/*; do printf '%-40s ' "$(basename "$f")"; vtool -arch arm64 -show-build "$f" | awk '/minos/ {print $2}'; done`
Expected: whisper-cli `13.0`, ffmpeg `12.0`, yt-dlp `11.0`.

Run it once more: `bash scripts/fetch-tools.sh 2>&1 | grep -A1 whisper-cli`
Expected: `already here`. The check that rebuilt it now leaves it alone.

- [ ] **Step 7: The changelog line for macOS 13**

In `CHANGELOG.md`, under `## Unreleased` > `### Fixed`, add after the line from Task 1:

```markdown
- Transcribing with Whisper and reading text off the screen work on macOS 13
  and later, not only on the newest macOS.
```

Run: `python3 scripts/changelog.py text Unreleased | tail -3`
Expected: the two Fixed lines.

- [ ] **Step 8: Commit**

```bash
git add scripts/fetch-tools.sh CHANGELOG.md
git commit -m "$(cat <<'EOF'
Build whisper-cli for the oldest macOS the app supports

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: release.sh proves both before it publishes

**Files:**
- Modify: `scripts/release.sh` (after the archive check, before `echo "== release v$version"`)

- [ ] **Step 1: Add the check**

Replace:

```bash
echo "== release v$version"
```

with:

```bash
# What a Mac that downloads this will hold against it, proved here rather than
# assumed. With only the linker's signature, which covers the binary but not
# the bundle, a downloaded copy is "damaged" and macOS offers no way through.
# A tool built for a newer macOS than the app promises fails there mid-job.
echo "== bundle"
app="$(ls -d "$bundle"/macos/*.app | head -1)"
if ! codesign --verify --deep --strict "$app" 2> /dev/null; then
  echo "   the bundle's signature does not hold. A downloaded copy would be 'damaged'."
  exit 1
fi
echo "   signature:  holds for the whole bundle"
minimum="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["bundle"]["macOS"]["minimumSystemVersion"])')"
for binary in "$app"/Contents/MacOS/*; do
  wants="$(vtool -arch arm64 -show-build "$binary" 2> /dev/null | awk '/minos/ {print $2; exit}' || true)"
  if ! python3 -c 'import sys; a, b = (tuple(map(int, v.split("."))) for v in sys.argv[1:]); sys.exit(a > b)' \
      "$wants" "$minimum" 2> /dev/null; then
    echo "   $(basename "$binary") asks for macOS ${wants:-?}; the app promises $minimum."
    exit 1
  fi
done
echo "   macOS:      every binary runs on $minimum"

echo "== release v$version"
```

- [ ] **Step 2: Syntax**

Run: `bash -n scripts/release.sh && echo "syntax ok"`
Expected: `syntax ok`. The check itself is run against a real build in Task 5.

- [ ] **Step 3: Commit**

```bash
git add scripts/release.sh
git commit -m "$(cat <<'EOF'
Check the signature and the macOS a release asks for before publishing it

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Build it, and check what a downloading Mac would check

**Files:** none changed, unless a check fails.

- [ ] **Step 1: Build**

Check no job is running. Then:

```bash
npm run build -- --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

The override only skips the updater archive, which needs the private key; the release builds it. Expected: the build ends listing `Tzz App.app` and `Tzz App_0.1.2_aarch64.dmg` under `src-tauri/target/release/bundle/`, with no signing error.

- [ ] **Step 2: The signature**

```bash
APP="src-tauri/target/release/bundle/macos/Tzz App.app"
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv --verbose=4 "$APP" 2>&1 | grep -E '^(Identifier|CodeDirectory|Signature|Sealed Resources|Info.plist)'
```

Expected: `valid on disk` and `satisfies its Designated Requirement`. `Identifier=com.nemanja.ytmp3`. The flags are `0x2(adhoc)`, without `linker-signed` and without `runtime`. `Signature=adhoc`. `Sealed Resources version=2`. `Info.plist entries=` followed by a number, not `not bound`.

- [ ] **Step 3: The macOS it asks for**

```bash
plutil -extract LSMinimumSystemVersion raw "$APP/Contents/Info.plist"
for f in "$APP"/Contents/MacOS/*; do printf '%-12s ' "$(basename "$f")"; vtool -arch arm64 -show-build "$f" | awk '/minos/ {print $2}'; done
vtool -arch arm64 -show-build "$(ls -t src-tauri/target/release/build/yt-mp3-*/out/ocr | head -1)" | awk '/minos/ {print "ocr helper  " $2}'
```

Expected: `13.0`; then `yt-mp3 13.0`, `whisper-cli 13.0`, `ffmpeg 12.0`, `yt-dlp 11.0`; then `ocr helper 13.0`.

- [ ] **Step 4: The tools still run after being signed again**

```bash
"$APP/Contents/MacOS/yt-dlp" --version
"$APP/Contents/MacOS/ffmpeg" -version | head -1
"$APP/Contents/MacOS/whisper-cli" --help > /dev/null 2>&1 && echo "whisper-cli runs"
```

Expected: a yt-dlp version, an ffmpeg version line, `whisper-cli runs`.

- [ ] **Step 5: As a downloaded copy**

```bash
S="/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/gatekeeper-new"
rm -rf "$S" && mkdir -p "$S"
ditto "$APP" "$S/Tzz App.app"
xattr -w com.apple.quarantine "0083;$(printf %x "$(date +%s)");Safari;" "$S/Tzz App.app"
spctl --assess --type execute -vv "$S/Tzz App.app"
```

Expected: `rejected` (it is not notarized), and no `code has no resources but signature indicates they must be present`, which is what 0.1.2 gave. That difference is "Open Anyway" versus "damaged".

- [ ] **Step 6: The disk image holds the same app**

```bash
M="/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/dmg-new"
mkdir -p "$M"
hdiutil attach -readonly -nobrowse -noautoopen -mountpoint "$M" src-tauri/target/release/bundle/dmg/*.dmg > /dev/null
codesign --verify --deep --strict "$M/Tzz App.app" && echo "dmg copy: signature holds"
hdiutil detach "$M" > /dev/null
```

Expected: `dmg copy: signature holds`.

- [ ] **Step 7: release.sh's new check, as written, on the new build and on 0.1.2**

```bash
C="/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/bundle-check"
rm -rf "$C" && mkdir -p "$C"
sed -n '/^echo "== bundle"$/,/every binary runs on/p' scripts/release.sh > "$C/check.sh"
bundle="src-tauri/target/release/bundle" bash -euo pipefail "$C/check.sh"; echo "exit $?"
```

Expected: `signature:  holds for the whole bundle`, `macOS:      every binary runs on 13.0`, `exit 0`.

Then against the 0.1.2 image downloaded earlier, at `/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/dmg/Tzz.App_0.1.2_aarch64.dmg` (fetch it again with `gh release download v0.1.2 -R NM193/tzz-app -p '*.dmg' -D <that folder>` if it is gone):

```bash
mkdir -p "$C/mnt" "$C/old/macos"
hdiutil attach -readonly -nobrowse -noautoopen -mountpoint "$C/mnt" "/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/dmg/Tzz.App_0.1.2_aarch64.dmg" > /dev/null
ditto "$C/mnt/Tzz App.app" "$C/old/macos/Tzz App.app"
hdiutil detach "$C/mnt" > /dev/null
bundle="$C/old" bash -euo pipefail "$C/check.sh"; echo "exit $?"
```

Expected: `the bundle's signature does not hold. A downloaded copy would be 'damaged'.` and `exit 1`.

- [ ] **Step 8: The owner tries it**

Ask the owner whether to put this build in place of `/Applications/Tzz App.app` (the copy steps of `scripts/install.sh`: quit the app, replace it, open it), then to make one short recording and one Whisper transcription. Expect macOS to ask for the microphone again: the signature it was granted to has changed. Do not release before the owner says both worked.

---

### Task 6: README.md, written again

**Files:**
- Rewrite: `README.md`

- [ ] **Step 1: Replace the whole file**

````markdown
# Tzz App

A macOS app that turns lectures into readable material. Give it a YouTube link,
a playlist, or an audio or video file from your Mac, and it makes an MP3, a
transcript with timestamps and chapters, and -- for screen-share videos -- the
text that was on screen. It can also record any audio input, including the
Mac's own sound.

The transcript is markdown and PDF, one folder per video in
`~/Documents/Tzz Library`. It is meant as a better source for NotebookLM, or
for whatever you read it with, than a bare YouTube link: it carries the
timestamps, the chapters and the screen text that a link does not.

## Getting it

Download the `.dmg` from the
[latest release](https://github.com/NM193/tzz-app/releases/latest), open it,
and drag **Tzz App** into Applications.

It needs a Mac with Apple Silicon (M1 or later) and macOS 13 or later.

### The first time you open it

The app is not signed by Apple, so macOS will not open it on the first try.

- **macOS 15 and later:** open it once and let macOS refuse. Then go to
  **System Settings → Privacy & Security**, scroll down to the line about Tzz
  App, and click **Open Anyway**.
- **macOS 13 and 14:** Control-click the app in Applications, choose **Open**,
  and confirm.

This is needed only once. Updates arrive inside the app afterwards.

### Before the first transcription

- **A Whisper model.** Videos with captions need nothing. For everything else
  -- local files, recordings, videos without captions -- the app transcribes
  on your Mac with Whisper. Open **Settings** and pick a model; the app fetches
  it. The large one is the best, especially for languages other than English.
- **BlackHole**, only to record the Mac's own sound, for example a call. It
  has to be installed by hand; **Record** links to it when it is missing.

### Updates

The app checks for a new version when it opens and says what changed. Nothing
is downloaded until you click **Update**. [CHANGELOG.md](CHANGELOG.md) has the
same notes.

## Building it

Needs: the Xcode command line tools (`xcode-select --install`, for `swiftc`),
Rust, Node, and cmake (`brew install cmake`).

```bash
npm install
bash scripts/fetch-tools.sh   # yt-dlp, ffmpeg and whisper-cli, inside the app
npm run dev                   # the app, with the frontend reloading
npm run build                 # a .app and a .dmg
```

`npm run build` makes the updater archive as well, which is signed with the
release key. Without that key, add
`-- --config '{"bundle":{"createUpdaterArtifacts":false}}'`.

How the code is laid out, how a release is made and why things are the way
they are: [CLAUDE.md](CLAUDE.md).

---

Downloading from YouTube goes against its terms of service. Sensible uses are
your own uploads, Creative Commons material, and transcripts for your own study
of things you have the right to watch.
````

- [ ] **Step 2: Check every claim in it against the code**

Run, and compare with the README:

```bash
grep -n 'Tzz Library' src-tauri/src/services/*.rs | head -3
grep -rn 'blackhole\|BlackHole' src/components/RecordView.tsx | head -3
grep -n 'minimumSystemVersion' src-tauri/tauri.conf.json
```

Expected: the output folder is `~/Documents/Tzz Library`; Record mentions BlackHole; the minimum is 13.0. If the model choice in Settings does not match "the large one", say what it does instead.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "$(cat <<'EOF'
Say how to get the app, not how it was built in August

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Write down why

**Files:**
- Modify: `CLAUDE.md` (section "Giving it to someone else")
- Modify (outside the repo): `/Users/nemanjamarjanovic/.claude/projects/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/memory/distribution-plan.md`
- Modify: `docs/superpowers/specs/2026-10-23-first-install-design.md` (status)

- [ ] **Step 1: CLAUDE.md**

Replace the whole bullet:

```markdown
- **The app is unsigned, and on macOS 15 and later that is not a right-click
  away.** Apple removed the Control-click override: the first open is refused
  outright, and the way through is System Settings -> Privacy & Security, where
  a line about the blocked app appears with **Open Anyway**. Only the first
  open, and only for a copy that came down through a browser. Writing
  "right-click -> Open" in the instructions sends someone to a menu item that
  no longer helps.
```

with:

```markdown
- **The app is signed ad hoc, not by Apple, and on macOS 15 and later that is
  not a right-click away.** Apple removed the Control-click override: the
  first open is refused outright, and the way through is System Settings ->
  Privacy & Security, where a line about the blocked app appears with **Open
  Anyway**. Only the first open, and only for a copy that came down through a
  browser. Writing "right-click -> Open" in the instructions sends someone to
  a menu item that no longer helps.
- **Ad hoc is not optional.** `bundle.macOS.signingIdentity` is `"-"`. Without
  it the bundle carries only the linker's signature, which covers the binary
  and not the bundle, and a downloaded copy is "damaged" with nothing offered
  in Privacy & Security -- 0.1.2 and earlier shipped that way. Hardened
  runtime is off: without notarization it buys nothing, and with it the
  microphone needs an entitlement, for the app and for the ffmpeg it records
  through.
- **The oldest macOS is one number**, `bundle.macOS.minimumSystemVersion`
  (13.0). Tauri writes it into `Info.plist` and builds the Rust for it.
  `build.rs` passes it to `swiftc` and `fetch-tools.sh` to cmake, because
  neither reads it on its own, and a tool built here without it asks for this
  Mac's macOS: 0.1.2's whisper-cli and OCR helper asked for 26. `release.sh`
  checks the signature and every binary against the number before it
  publishes.
```

- [ ] **Step 2: The memory**

In the memory file, replace the heading line:

```markdown
**First install from the DMG, measured 2026-10-23 on the published v0.1.2**
(the owner was told; no fix decided yet):
```

with:

```markdown
**First install from the DMG, measured 2026-10-23 on the published v0.1.2.**
Fixed on main the same day (ad hoc signing, minimum macOS 13.0, both checked
by release.sh); ships with 0.1.3. What was found:
```

- [ ] **Step 3: The spec's status**

Replace `**Status:** agreed 2026-10-23, to ship in 0.1.3.` with `**Status:** built 2026-10-23, ships in 0.1.3.`

- [ ] **Step 4: Everything that has a test**

Run the test-runner agent: `python3 scripts/test_changelog.py`, `node --test src/lib/releaseNotes.test.ts`, `npm run build:vite`, `bash -n scripts/release.sh scripts/fetch-tools.sh`, `cd src-tauri && cargo test`.
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md docs/superpowers/specs/2026-10-23-first-install-design.md docs/superpowers/plans/2026-10-23-first-install.md
git commit -m "$(cat <<'EOF'
Write down why the bundle is signed and which macOS it asks for

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```
