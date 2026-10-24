# Recording Without Gaps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recordings of the Mac's sound and of the microphone come out whole: no
dropped slices, no speed-up, no garble.

**Architecture:** The app reads each input itself with a CoreAudio AudioQueue
and pipes plain 32-bit float samples to ffmpeg, which only mixes, meters and
encodes. Before recording the Mac's sound, the app sets the capture device
(BlackHole) to the Multi-Output Device's sample rate.

**Tech Stack:** Rust, hand-written CoreAudio/AudioToolbox FFI (the style of
`services/audio_output.rs`), ffmpeg 7.1.1, `std::io::pipe` (Rust 1.87+).

---

## What was found (measured 2026-10-24)

A three-tone test signal (997, 1451, 1925 Hz) was played while the installed
app recorded, and each 10 ms of the result was traced back to the sample of the
original it came from. Two separate faults, both in 0.1.2 and 0.1.3:

1. **BlackHole at 44.1 kHz, the Multi-Output Device at 48 kHz.** A
   Multi-Output Device does not set its sub-devices' rates, and drift
   correction (on, quality 127) cannot bridge 9%. The recording came out
   x1.09 in pitch, x2.3 in level, garbled. With BlackHole at 48 kHz: pitch
   1.000, level 2519 against 2508 played.
2. **ffmpeg's avfoundation input drops whole buffers.** It holds one audio
   buffer and releases an unread one when the next arrives
   (`libavdevice/avfoundation.m:276-280`, n7.1.1), and its demuxer sleeps
   10 ms whenever nothing is waiting (`fftools/ffmpeg_demux.c:708-709`). At
   48 kHz CoreAudio delivers 512 frames every 10.67 ms, so about one buffer in
   ten is overwritten: skips of exactly 512 samples, 6-10% of the audio gone
   (21.65 s recorded, 19.48 s in the file). Identical with the tone played
   straight into BlackHole, so the Multi-Output Device plays no part. The
   microphone runs at 48 kHz with 512 frames too; not measured.

## Decided

The owner chose to fix both in the app, and to ship it as 0.1.4.

- **Rate first.** `tray.rs` sets the input device to the Multi-Output Device's
  nominal rate *before* `recorder::start`. Today ffmpeg is started first and
  the output switched after; a rate changed under a running capture changes
  its format mid-stream. A failure is reported in a plain sentence and the
  recording goes ahead.
- **Own capture, in process.** `services/capture.rs` lists input devices
  through CoreAudio and records one with an AudioQueue: Float32 interleaved,
  the device's own rate, its channel count capped at two, ten 100 ms buffers,
  the callback on the queue's own thread (not the HAL's real-time one) writing
  into a pipe. Rejected: a Swift helper (one more binary, and it outlives a
  killed app); an IOProc (real-time thread, needs a lock-free ring); and
  ScreenCaptureKit, which would retire BlackHole but needs Screen Recording
  permission and leaves the microphone unsolved -- a separate decision.
- **ffmpeg reads samples, not devices.** `-f f32le -ar R -ac C -i pipe:0`, and
  the second input `-i pipe:3` through `std::io::pipe` and `dup2` in
  `pre_exec`. The filter graph stays as it is. Stopping means stopping the
  queues and closing the pipes; ffmpeg finishes the file at end of input, so
  the `q` on stdin goes. A killed app takes its queues with it, the pipes
  close, and ffmpeg finishes the file by itself; `stop_orphan` stays as a
  backstop.
- **`AudioInput { index, name }` keeps its shape**, so `api.ts` does not
  change; `index` becomes the CoreAudio device id. The window re-picks from
  the list whenever it loads it (`useRecorder.ts:60`) and nothing is stored,
  so no old index goes stale. `tray.rs`'s `unwrap_or(0)` becomes "the first
  input", since device id 0 means none.
- The avfoundation listing parser goes, with its tests.

## Files

- Modify `src-tauri/src/services/audio_output.rs` -- nominal rate, input
  channel count, device UID; `all_devices` and `device_name` become
  `pub(crate)` for the capture module.
- Create `src-tauri/src/services/capture.rs` -- input listing, AudioQueue
  capture into a writer.
- Modify `src-tauri/src/services/mod.rs` -- `pub mod capture;`.
- Modify `src-tauri/src/services/recorder.rs` -- ffmpeg reads pipes; start,
  stop, listing.
- Modify `src-tauri/src/services/tray.rs` -- rate before start; first input
  instead of 0.
- Modify `CHANGELOG.md`, `CLAUDE.md`, `README.md`.

---

### Task 1: The capture device follows the Multi-Output Device's rate

**Files:** `services/audio_output.rs`, `services/tray.rs`, `CHANGELOG.md`

- [ ] **Step 1: Test the new code points** (in `audio_output.rs` tests)

```rust
#[test]
fn nominal_rate_and_uid_codes_match_coreaudio() {
    assert_eq!(kAudioDevicePropertyNominalSampleRate, 0x6E73_7274); // 'nsrt'
    assert_eq!(kAudioDevicePropertyDeviceUID, 0x7569_6420); // 'uid '
    assert_eq!(kAudioObjectPropertyScopeInput, 0x696E_7074); // 'inpt'
}
```

- [ ] **Step 2: `cd src-tauri && cargo test nominal_rate`** -- fails: the
  constants do not exist.
- [ ] **Step 3: Implement** `nominal_rate(device) -> Option<f64>`,
  `set_nominal_rate(device, rate) -> Result<(), String>` (`'nsrt'`, global
  scope, `f64`), the constants above, and `id_named(name) -> Option<DeviceId>`
  over `all_devices()`.
- [ ] **Step 4: In `tray.rs`**, a `match_rate_for(app, input)` called before
  `recorder::start` when `captures_system_audio(index)`: find the Multi-Output
  Device, read both rates, set the input's when they differ; on failure emit
  `recording://error` with "BlackHole runs at {a} kHz and the Multi-Output
  Device at {b} kHz, and the app could not change it. Set both to the same
  rate in Audio MIDI Setup." The input is still named by avfoundation's
  listing here, so the device is found by name; Task 3 makes it an id.
- [ ] **Step 5: `cargo test`** -- all pass.
- [ ] **Step 6: CHANGELOG**, under Unreleased, Fixed: "Recording the Mac's
  sound no longer comes out too fast and garbled when BlackHole and the
  Multi-Output Device run at different sample rates: the app matches them
  before it starts."
- [ ] **Step 7: Commit** -- "Match BlackHole's rate to the Multi-Output Device
  before recording".

### Task 2: Capture an input with an AudioQueue

**Files:** create `services/capture.rs`; `services/mod.rs`;
`services/audio_output.rs` (`pub(crate)`, input channels, UID)

- [ ] **Step 1: Tests first** (in `capture.rs`)

```rust
#[test]
fn layouts_match_the_c_structs() {
    assert_eq!(std::mem::size_of::<AudioStreamBasicDescription>(), 40);
    assert_eq!(std::mem::size_of::<AudioQueueBuffer>(), 56);
}

#[test]
fn asks_for_interleaved_float_at_the_device_rate() {
    let f = float_format(48_000.0, 2);
    assert_eq!(f.format_id, fourcc(b"lpcm"));
    assert_eq!(f.format_flags, FLAG_IS_FLOAT | FLAG_IS_PACKED);
    assert_eq!((f.bytes_per_frame, f.bytes_per_packet, f.frames_per_packet), (8, 8, 1));
    assert_eq!((f.channels_per_frame, f.bits_per_channel), (2, 32));
}

#[test]
fn never_asks_for_more_than_two_channels() {
    assert_eq!(channels_for(16), 2);
    assert_eq!(channels_for(1), 1);
}

#[test]
fn current_device_code_matches_audiotoolbox() {
    assert_eq!(kAudioQueueProperty_CurrentDevice, 0x6171_6364); // 'aqcd'
}
```

- [ ] **Step 2: `cargo test capture`** -- fails: module missing.
- [ ] **Step 3: Implement.** FFI, linked against AudioToolbox:
  `AudioQueueNewInput`, `AudioQueueAllocateBuffer`, `AudioQueueEnqueueBuffer`,
  `AudioQueueStart`, `AudioQueueStop`, `AudioQueueDispose`,
  `AudioQueueSetProperty`. `#[repr(C)]` structs:

```rust
#[repr(C)]
pub struct AudioStreamBasicDescription {
    sample_rate: f64, format_id: u32, format_flags: u32, bytes_per_packet: u32,
    frames_per_packet: u32, bytes_per_frame: u32, channels_per_frame: u32,
    bits_per_channel: u32, reserved: u32,
}

#[repr(C)]
struct AudioQueueBuffer {
    capacity: u32, data: *mut c_void, byte_size: u32, user_data: *mut c_void,
    packet_capacity: u32, packets: *mut c_void, packet_count: u32,
}
```

  Public surface:

```rust
pub struct Format { pub rate: f64, pub channels: u32 }
pub fn inputs() -> Vec<(DeviceId, String)>          // devices with input channels
pub fn format_of(device: DeviceId) -> Option<Format> // nominal rate, channels_for(n)
pub struct Capture { /* queue, Box<Sink> */ }
impl Capture {
    pub fn start(device: DeviceId, out: Box<dyn Write + Send>) -> Result<(Capture, Format), String>;
    pub fn stop(self);                               // stop, dispose, close `out`
}
```

  `start`: format from `format_of`; `AudioQueueNewInput` with the callback,
  a `Box<Sink>` as user data, null run loop; set `'aqcd'` to the device's UID
  (a CFString, released after); ten buffers of 100 ms, enqueued;
  `AudioQueueStart`. The callback writes `byte_size` bytes from `data` into
  the sink and re-enqueues the buffer while the sink is open; a write error
  (ffmpeg gone) closes the sink and stops re-enqueuing. No `unwrap`, nothing
  that can panic: a panic out of an `extern "C"` function aborts the app.
  `stop`: mark the sink closed, `AudioQueueStop(queue, true)` (synchronous,
  no callback after it returns), `AudioQueueDispose(queue, true)`, then drop
  the sink, which closes the pipe.
- [ ] **Step 4: `cargo test`** -- all pass.
- [ ] **Step 5: Commit** -- "Read an audio input through an AudioQueue".

### Task 3: ffmpeg encodes what the app captured

**Files:** `services/recorder.rs`, `services/tray.rs`, `CHANGELOG.md`

- [ ] **Step 1: Tests first** (in `recorder.rs`; `filter_for` tests stay)

```rust
#[test]
fn one_input_arrives_on_stdin_as_raw_float() {
    let args = input_args(&[Format { rate: 48_000.0, channels: 2 }]);
    assert_eq!(args, ["-f", "f32le", "-ar", "48000", "-ac", "2", "-i", "pipe:0"]);
}

#[test]
fn a_second_input_arrives_on_descriptor_three() {
    let args = input_args(&[
        Format { rate: 48_000.0, channels: 2 },
        Format { rate: 48_000.0, channels: 1 },
    ]);
    assert_eq!(&args[8..], ["-f", "f32le", "-ar", "48000", "-ac", "1", "-i", "pipe:3"]);
}
```

- [ ] **Step 2: `cargo test input_args`** -- fails.
- [ ] **Step 3: Implement.** `list_inputs()` maps `capture::inputs()` to
  `AudioInput`. `start()`: formats for both devices; ffmpeg with
  `input_args`, the unchanged filter, `-map [a] -c:a libmp3lame -q:a 0`,
  `stdin(Stdio::piped())`; with a second input, `std::io::pipe()` and, in
  `pre_exec`, `dup2(reader, 3)` (declared `extern "C"`, it is in libSystem),
  the reader dropped in the parent after spawn. Then `Capture::start` for
  each, into `child.stdin.take()` and the pipe writer; if either fails, stop
  what started and kill ffmpeg. `Recording` holds the captures. `stop()`:
  stop the captures (that closes the pipes), then `child.wait()`; the `q`
  goes. The avfoundation parser and its two tests go. Module doc: why the
  app reads inputs itself, with the measured numbers.
- [ ] **Step 4: `tray.rs`** -- `selected_input(app)` falls back to the first
  listed input, not 0; `match_rate_for` takes the device id directly.
- [ ] **Step 5: `cargo test`** and `npm run build:vite` -- all pass.
- [ ] **Step 6: CHANGELOG**, Fixed: "Recordings no longer lose a sliver of
  sound about ten times a second."
- [ ] **Step 7: Commit** -- "Record through the app's own capture, not
  ffmpeg's".

### Task 4: Write it down

**Files:** `CLAUDE.md`, `README.md`

- [ ] **Step 1: CLAUDE.md**, Lessons: "**ffmpeg's avfoundation input drops
  audio.** It keeps one buffer and looks every 10 ms; at 48 kHz a buffer
  arrives every 10.7 ms and about one in ten was overwritten. The app reads
  inputs itself through an AudioQueue and hands ffmpeg plain samples." and
  "**BlackHole runs at the Multi-Output Device's rate, or the recording is
  garbage.** A Multi-Output Device does not set its sub-devices' rates, and
  drift correction cannot bridge 44.1 to 48 kHz. The app sets it before
  recording." Services list: "`capture` reading an audio input".
- [ ] **Step 2: README**, the BlackHole bullet: after installing it, a
  Multi-Output Device in Audio MIDI Setup with your speakers and BlackHole;
  the app switches to it while recording and back afterwards.
- [ ] **Step 3: Commit** -- "Write down why the app reads its inputs itself".

### Task 5: Prove it on the real thing

- [x] **Step 1: test-runner** -- cargo test, build:vite, the Python and Node
  tests.
- [x] **Step 2: Ask the owner**, then `npm run install:app`.
- [x] **Step 3: The three-tone test** through the installed app (watcher
  pattern `MacOS/ffmpeg .*-f f32le` -- the args start with `-probesize`),
  parts A, B and C as on 2026-10-24.
  Expected: no seams in any part, recorded length equal to the wall clock
  within 0.1 s, pitch 1.0000, level about 2508.
  Result: BlackHole set to 44.1 kHz beforehand, the app moved it to 48 kHz;
  pitch 1.0000; the 5 s tone measured 4.6996 s between its half-amplitude
  fades against 4.6999 s played; length against the wall clock +0.07 s.
  Only part A reached BlackHole: a monitor plugged in moved the audiotoolbox
  indices, so B and C went elsewhere. Look devices up by name next time.
- [x] **Step 4: The microphone** -- the owner records ten seconds of speech
  and listens; length against the wall clock. Result: +0.06 s, sounds right.
- [x] **Step 5: Two inputs** -- BlackHole and the microphone together;
  length against the wall clock, and the owner listens. Result: +0.09 s,
  BlackHole again moved from 44.1 to 48 kHz, sounds right.
- [x] **Step 6: reviewer** on the diff.
