# Audio recorder

**Status:** approved 2026-08-29

## Problem

Recording something worth transcribing -- a call, a lecture playing on screen --
currently means leaving the app: record with macOS, find the file, drag it in.
The app already does everything after that step.

## What we build

A recording bar pinned above the link field, visible at all times. Pick an
input, press Record, press Stop, give the file a name. It lands in the list
marked as saved but not transcribed, and waits for you to say when.

Out of scope: video capture, pause and resume, mixing several inputs.

## Why audio only

macOS has no loopback audio device. `ffmpeg -f avfoundation -list_devices`
reports exactly one input on this machine, `MacBook Pro Microphone`, and the
built-in `screencapture -g` records "using default input" -- also the
microphone. Capturing what the speakers play needs a virtual audio driver.

So the app does not decide what can be recorded: it lists whatever inputs the
system reports. Today that is the microphone. Installing BlackHole
(`brew install --cask blackhole-2ch`, plus a Multi-Output Device in Audio MIDI
Setup so sound is still audible) makes system audio appear in the same list,
with no change to this code.

## The recorder is independent of the queue

Recording can start while a transcription is running and vice versa. Whisper
saturates the GPU; ffmpeg writing an MP3 does not compete with it. Blocking one
on the other would only be a limitation invented for the user.

This is also why the bar is pinned at the top rather than living inside the
queue: it belongs to neither the current job nor the current list.

## First long-lived state in the app

Every command so far has been enter, do, leave. Recording spans two commands,
so the child process has to live between them: a Tauri-managed
`Mutex<Option<Recording>>` holding the ffmpeg child, the temp path and the
start time.

Stopping writes `q` to ffmpeg's stdin rather than killing the process. ffmpeg
then finalises the MP3 properly; a killed process leaves a file that will not
open.

## Commands

    list_audio_inputs() -> Vec<AudioInput>
      Parses `ffmpeg -f avfoundation -list_devices true -i ""`.

    start_recording(inputIndex) -> ()
      Refuses if a recording is already running. Writes to a temp file.

    stop_recording() -> RecordingResult { tempPath, seconds }
      Graceful stop. The file stays in temp until it is named.

    save_recording(tempPath, destination) -> String
      Moves the finished file where the user chose.

## Naming

On stop, the native save dialog opens -- the same plugin the folder picker
already uses -- with a date-and-time default. Naming after the fact is
deliberate: you know what the recording was only once it is over.

If the dialog is cancelled the file stays in temp and the app says where, so a
mis-click never destroys an hour of audio.

## The saved state

`QueueStatus` gains `saved`: present, named, not transcribed. Until now
`waiting` meant "queued and will run", which is not what a recording is.

Rows in that state show a **Transcribe** button and nothing happens until it is
pressed. Pressing it moves the item into the normal queue as a file job.

## Permission

macOS needs `NSMicrophoneUsageDescription` in Info.plist or the app silently
records silence. Because the bundle is not signed with a Developer ID, macOS
may treat each rebuild as a different app and ask again.

## Testing

`cargo test` covers the pure parts:
- parsing the ffmpeg device list, including no audio devices at all
- the default recording name

Manual: record, stop, name it, confirm the MP3 plays and is the right length;
cancel the save dialog and confirm the file is still reported; press Transcribe
and confirm it runs as an ordinary file job; start a recording while the queue
is transcribing and confirm neither disturbs the other.
