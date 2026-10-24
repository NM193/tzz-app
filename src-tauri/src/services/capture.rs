//! Reading an audio input, so ffmpeg only has to encode it.
//!
//! WHY the app reads inputs itself: ffmpeg's avfoundation input holds one
//! buffer at a time and looks for the next every 10 ms. At 48 kHz CoreAudio
//! hands over 512 frames every 10.67 ms, so about one buffer in ten was
//! overwritten before ffmpeg read it -- a slice of sound gone ten times a
//! second, 6-10% of every recording (measured 2026-10-24). An AudioQueue keeps
//! a second's worth of buffers instead, and what it fills goes down a pipe.
//!
//! The queue calls back on a thread of its own, not on CoreAudio's real-time
//! one, so the callback may write to a pipe and wait on it a moment.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::c_void;
use std::io::{ErrorKind, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use super::audio_output::{self, fourcc, DeviceId};

const kAudioFormatLinearPCM: u32 = fourcc(b"lpcm");
const FLAG_IS_FLOAT: u32 = 1 << 0;
const FLAG_IS_PACKED: u32 = 1 << 3;
const kAudioQueueProperty_CurrentDevice: u32 = fourcc(b"aqcd");

/// A second of slack before anything is lost, in buffers small enough that
/// stopping drops at most the last 50 ms.
const BUFFER_SECONDS: f64 = 0.05;
const BUFFERS: usize = 20;

#[repr(C)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[repr(C)]
struct AudioQueueBuffer {
    capacity: u32,
    data: *mut c_void,
    byte_size: u32,
    user_data: *mut c_void,
    packet_capacity: u32,
    packets: *mut c_void,
    packet_count: u32,
}

type AudioQueueRef = *mut c_void;

type InputCallback = extern "C" fn(
    user_data: *mut c_void,
    queue: AudioQueueRef,
    buffer: *mut AudioQueueBuffer,
    start_time: *const c_void,
    packet_count: u32,
    packets: *const c_void,
);

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioQueueNewInput(
        format: *const AudioStreamBasicDescription,
        callback: InputCallback,
        user_data: *mut c_void,
        run_loop: *const c_void,
        run_loop_mode: *const c_void,
        flags: u32,
        queue: *mut AudioQueueRef,
    ) -> i32;
    fn AudioQueueSetProperty(queue: AudioQueueRef, id: u32, data: *const c_void, size: u32) -> i32;
    fn AudioQueueAllocateBuffer(queue: AudioQueueRef, bytes: u32, buffer: *mut *mut AudioQueueBuffer) -> i32;
    fn AudioQueueEnqueueBuffer(
        queue: AudioQueueRef,
        buffer: *mut AudioQueueBuffer,
        packet_count: u32,
        packets: *const c_void,
    ) -> i32;
    fn AudioQueueStart(queue: AudioQueueRef, start_time: *const c_void) -> i32;
    fn AudioQueueStop(queue: AudioQueueRef, immediate: u8) -> i32;
    fn AudioQueueDispose(queue: AudioQueueRef, immediate: u8) -> i32;
}

/// What a capture delivers: 32-bit float samples, interleaved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Format {
    pub rate: f64,
    pub channels: u32,
}

/// Every device that can record, with its name.
pub fn inputs() -> Vec<(DeviceId, String)> {
    audio_output::all_devices()
        .into_iter()
        .filter(|&device| audio_output::input_channels(device) > 0)
        .filter_map(|device| audio_output::name_of(device).map(|name| (device, name)))
        .collect()
}

/// The device's own rate, so nothing is resampled on the way in.
pub fn format_of(device: DeviceId) -> Option<Format> {
    let rate = audio_output::nominal_rate(device)?;
    let channels = channels_for(audio_output::input_channels(device));
    (channels > 0).then_some(Format { rate, channels })
}

/// An MP3 holds two channels. More is a BlackHole 16ch or an interface, whose
/// first two are the ones anything is routed to by default.
fn channels_for(device_channels: u32) -> u32 {
    device_channels.min(2)
}

fn float_format(rate: f64, channels: u32) -> AudioStreamBasicDescription {
    let bytes = 4 * channels;
    AudioStreamBasicDescription {
        sample_rate: rate,
        format_id: kAudioFormatLinearPCM,
        format_flags: FLAG_IS_FLOAT | FLAG_IS_PACKED,
        bytes_per_packet: bytes,
        frames_per_packet: 1,
        bytes_per_frame: bytes,
        channels_per_frame: channels,
        bits_per_channel: 32,
        reserved: 0,
    }
}

/// Where the callback writes, and whether it still should.
struct Sink {
    out: Mutex<Option<Box<dyn Write + Send>>>,
    open: AtomicBool,
}

/// One input being recorded. Stopping it, or dropping it, closes the pipe, so
/// whatever reads the other end sees the end of the recording.
pub struct Capture {
    queue: AudioQueueRef,
    sink: *mut Sink,
}

// The queue is a CoreAudio object, safe to stop from any thread, and the sink
// is only touched through its Mutex and atomic.
unsafe impl Send for Capture {}

impl Capture {
    /// Record `device` into `out` as `format` -- the format ffmpeg was told to
    /// expect. Should the device have moved to another rate since, the queue
    /// converts, so what arrives is still what ffmpeg reads it as.
    pub fn start(device: DeviceId, format: Format, out: Box<dyn Write + Send>) -> Result<Capture, String> {
        let uid = audio_output::uid(device).ok_or("That input is no longer there.")?;
        let description = float_format(format.rate, format.channels);
        let sink = Box::into_raw(Box::new(Sink { out: Mutex::new(Some(out)), open: AtomicBool::new(true) }));

        let mut queue: AudioQueueRef = std::ptr::null_mut();
        let status = unsafe {
            AudioQueueNewInput(
                &description,
                on_filled,
                sink as *mut c_void,
                std::ptr::null(),
                std::ptr::null(),
                0,
                &mut queue,
            )
        };
        if status != 0 {
            drop(unsafe { Box::from_raw(sink) });
            return Err(format!("Could not open that input (CoreAudio error {status})."));
        }
        // From here on, dropping `capture` stops the queue and frees the sink.
        let capture = Capture { queue, sink };

        let device_uid = uid.as_cf_string();
        let status = unsafe {
            AudioQueueSetProperty(
                queue,
                kAudioQueueProperty_CurrentDevice,
                &device_uid as *const _ as *const c_void,
                std::mem::size_of::<*const c_void>() as u32,
            )
        };
        if status != 0 {
            return Err(format!("Could not record from that input (CoreAudio error {status})."));
        }

        let bytes = (format.rate * BUFFER_SECONDS) as u32 * description.bytes_per_frame;
        for _ in 0..BUFFERS {
            let mut buffer: *mut AudioQueueBuffer = std::ptr::null_mut();
            let status = unsafe { AudioQueueAllocateBuffer(queue, bytes, &mut buffer) };
            if status != 0 {
                return Err(format!("Could not set up the recording (CoreAudio error {status})."));
            }
            unsafe { AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null()) };
        }

        let status = unsafe { AudioQueueStart(queue, std::ptr::null()) };
        if status != 0 {
            return Err(format!("Could not start recording that input (CoreAudio error {status})."));
        }

        Ok(capture)
    }

    pub fn stop(self) {
        drop(self);
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            (*self.sink).open.store(false, Ordering::SeqCst);
            // Immediate means synchronous: no callback runs once these return,
            // so the sink can go.
            AudioQueueStop(self.queue, 1);
            AudioQueueDispose(self.queue, 1);
            drop(Box::from_raw(self.sink));
        }
    }
}

/// A buffer is full: send it down the pipe and hand it back to the queue.
///
/// Nothing here may panic -- a panic out of an `extern "C"` function aborts the
/// app, recording and all -- so every failure just closes the sink.
extern "C" fn on_filled(
    user_data: *mut c_void,
    queue: AudioQueueRef,
    buffer: *mut AudioQueueBuffer,
    _start_time: *const c_void,
    _packet_count: u32,
    _packets: *const c_void,
) {
    let sink = unsafe { &*(user_data as *const Sink) };
    if !sink.open.load(Ordering::SeqCst) {
        return;
    }

    let (data, size) = unsafe { ((*buffer).data as *const u8, (*buffer).byte_size as usize) };
    let delivered = size == 0 || deliver(sink, unsafe { std::slice::from_raw_parts(data, size) });

    if delivered {
        unsafe { AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null()) };
    } else {
        // Nothing reads the other end any more: ffmpeg has gone.
        sink.open.store(false, Ordering::SeqCst);
    }
}

/// Write all of `bytes`, waiting out a full pipe for as long as the capture
/// is open.
///
/// The pipe never blocks -- recorder.rs makes it so. A write that blocked
/// would hold up the queue's thread, and stopping the queue waits for that
/// thread: if ffmpeg stopped reading, as it does when a second input goes
/// quiet and holds up the mix, Stop would never return. This way a stop gets
/// through within a couple of milliseconds.
fn deliver(sink: &Sink, mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        let written = match sink.out.lock() {
            Ok(mut out) => match out.as_mut() {
                Some(out) => out.write(bytes),
                None => return false,
            },
            Err(_) => return false,
        };
        match written {
            Ok(0) => return false,
            Ok(n) => bytes = bytes.get(n..).unwrap_or_default(),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if !sink.open.load(Ordering::SeqCst) {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_the_c_structs() {
        assert_eq!(std::mem::size_of::<AudioStreamBasicDescription>(), 40);
        assert_eq!(std::mem::size_of::<AudioQueueBuffer>(), 56);
    }

    #[test]
    fn asks_for_interleaved_float_at_the_device_rate() {
        let f = float_format(48_000.0, 2);
        assert_eq!(f.sample_rate, 48_000.0);
        assert_eq!(f.format_id, 0x6C70_636D); // 'lpcm'
        assert_eq!(f.format_flags, FLAG_IS_FLOAT | FLAG_IS_PACKED);
        assert_eq!((f.bytes_per_frame, f.bytes_per_packet, f.frames_per_packet), (8, 8, 1));
        assert_eq!((f.channels_per_frame, f.bits_per_channel), (2, 32));
    }

    #[test]
    fn never_asks_for_more_than_two_channels() {
        assert_eq!(channels_for(16), 2);
        assert_eq!(channels_for(2), 2);
        assert_eq!(channels_for(1), 1);
        assert_eq!(channels_for(0), 0);
    }

    #[test]
    fn current_device_code_matches_audiotoolbox() {
        assert_eq!(kAudioQueueProperty_CurrentDevice, 0x6171_6364); // 'aqcd'
    }

    /// A pipe that is full a few times, then takes three bytes per write.
    struct Pipe {
        full_for: usize,
        broken: bool,
        taken: std::sync::Arc<Mutex<Vec<u8>>>,
    }

    impl Write for Pipe {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.broken {
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }
            if self.full_for > 0 {
                self.full_for -= 1;
                return Err(std::io::ErrorKind::WouldBlock.into());
            }
            let n = bytes.len().min(3);
            self.taken.lock().unwrap().extend_from_slice(&bytes[..n]);
            Ok(n)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn sink(pipe: Pipe, open: bool) -> Sink {
        Sink { out: Mutex::new(Some(Box::new(pipe))), open: AtomicBool::new(open) }
    }

    #[test]
    fn a_full_pipe_is_waited_out_until_it_takes_everything() {
        let taken = std::sync::Arc::new(Mutex::new(Vec::new()));
        let sink = sink(Pipe { full_for: 2, broken: false, taken: taken.clone() }, true);
        assert!(deliver(&sink, b"abcdefgh"));
        assert_eq!(&*taken.lock().unwrap(), b"abcdefgh");
    }

    #[test]
    fn a_full_pipe_is_given_up_on_once_the_capture_stops() {
        let sink = sink(Pipe { full_for: usize::MAX, broken: false, taken: Default::default() }, false);
        assert!(!deliver(&sink, b"abc"));
    }

    #[test]
    fn a_closed_pipe_ends_the_capture() {
        let sink = sink(Pipe { full_for: 0, broken: true, taken: Default::default() }, true);
        assert!(!deliver(&sink, b"abc"));
    }
}
