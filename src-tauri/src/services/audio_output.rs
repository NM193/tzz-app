//! Switching the Mac's output device.
//!
//! WHY this exists: capturing what the speakers play means routing output
//! through a Multi-Output Device that feeds BlackHole. Leaving it selected all
//! the time breaks the volume keys, so the app switches to it when recording
//! starts and puts the old device back when recording stops.
//!
//! macOS has no command for this, so it is done through CoreAudio directly.
//! That keeps it to one file with no extra crate and nothing for the user to
//! install. The device properties the recorder reads an input by -- its rate,
//! its channels, its UID -- come from here too.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::{c_char, c_void, CStr};

/// Four-character codes, as CoreAudio spells them.
const kAudioObjectSystemObject: u32 = 1;
const kAudioHardwarePropertyDefaultOutputDevice: u32 = fourcc(b"dOut");
const kAudioHardwarePropertyDefaultSystemOutputDevice: u32 = fourcc(b"sOut");
const kAudioHardwarePropertyDevices: u32 = fourcc(b"dev#");
const kAudioObjectPropertyName: u32 = fourcc(b"lnam");
const kAudioDevicePropertyStreamConfiguration: u32 = fourcc(b"slay");
const kAudioDevicePropertyNominalSampleRate: u32 = fourcc(b"nsrt");
const kAudioDevicePropertyDeviceUID: u32 = fourcc(b"uid ");
const kAudioObjectPropertyScopeGlobal: u32 = fourcc(b"glob");
const kAudioObjectPropertyScopeInput: u32 = fourcc(b"inpt");
const kAudioObjectPropertyScopeOutput: u32 = fourcc(b"outp");
const kAudioObjectPropertyElementMain: u32 = 0;
const kCFStringEncodingUTF8: u32 = 0x0800_0100;

pub(crate) const fn fourcc(code: &[u8; 4]) -> u32 {
    ((code[0] as u32) << 24) | ((code[1] as u32) << 16) | ((code[2] as u32) << 8) | code[3] as u32
}

#[repr(C)]
struct PropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

impl PropertyAddress {
    const fn global(selector: u32) -> Self {
        Self { selector, scope: kAudioObjectPropertyScopeGlobal, element: kAudioObjectPropertyElementMain }
    }
}

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        out_size: *mut u32,
    ) -> i32;

    fn AudioObjectGetPropertyData(
        object: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        io_size: *mut u32,
        out_data: *mut c_void,
    ) -> i32;

    fn AudioObjectSetPropertyData(
        object: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        data_size: u32,
        data: *const c_void,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetCString(string: *const c_void, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
    fn CFRelease(cf: *const c_void);
}

pub type DeviceId = u32;

/// The device the Mac is playing through right now.
pub fn current_output() -> Option<DeviceId> {
    read_device(kAudioHardwarePropertyDefaultOutputDevice)
}

fn read_device(selector: u32) -> Option<DeviceId> {
    let address = PropertyAddress::global(selector);
    let mut device: DeviceId = 0;
    let mut size = std::mem::size_of::<DeviceId>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut device as *mut _ as *mut c_void,
        )
    };

    (status == 0 && device != 0).then_some(device)
}

/// Where alert sounds go.
///
/// macOS keeps this on real hardware even while the main output is a
/// Multi-Output Device, which makes it a dependable answer to "what should we
/// go back to" when the app has nothing remembered.
pub fn system_output() -> Option<DeviceId> {
    read_device(kAudioHardwarePropertyDefaultSystemOutputDevice)
}

pub fn name_of(device: DeviceId) -> Option<String> {
    device_name(device)
}

pub fn set_output(device: DeviceId) -> Result<(), String> {
    let address = PropertyAddress::global(kAudioHardwarePropertyDefaultOutputDevice);

    let status = unsafe {
        AudioObjectSetPropertyData(
            kAudioObjectSystemObject,
            &address,
            0,
            std::ptr::null(),
            std::mem::size_of::<DeviceId>() as u32,
            &device as *const _ as *const c_void,
        )
    };

    if status == 0 {
        Ok(())
    } else {
        Err(format!("Could not change the sound output (CoreAudio error {status})."))
    }
}

/// The sample rate a device runs at, in Hz.
pub fn nominal_rate(device: DeviceId) -> Option<f64> {
    let address = PropertyAddress::global(kAudioDevicePropertyNominalSampleRate);
    let mut rate: f64 = 0.0;
    let mut size = std::mem::size_of::<f64>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut rate as *mut _ as *mut c_void,
        )
    };

    (status == 0 && rate > 0.0).then_some(rate)
}

/// Run a device at another sample rate.
///
/// WHY: a Multi-Output Device does not set the rate of the devices inside it.
/// With BlackHole at 44.1 kHz and the Multi-Output Device at 48 kHz, drift
/// correction cannot bridge the 9%, and the recording came out 9% fast, twice
/// as loud and garbled.
pub fn set_nominal_rate(device: DeviceId, rate: f64) -> Result<(), String> {
    let address = PropertyAddress::global(kAudioDevicePropertyNominalSampleRate);

    let status = unsafe {
        AudioObjectSetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            std::mem::size_of::<f64>() as u32,
            &rate as *const _ as *const c_void,
        )
    };

    if status == 0 {
        Ok(())
    } else {
        Err(format!("Could not change the sample rate (CoreAudio error {status})."))
    }
}

/// Every device that can play sound, with its name.
pub fn outputs() -> Vec<(DeviceId, String)> {
    all_devices()
        .into_iter()
        .filter(|&device| has_output_channels(device))
        .filter_map(|device| device_name(device).map(|name| (device, name)))
        .collect()
}

/// The device that also feeds a capture driver, if the user has made one.
///
/// Named by convention rather than configured: macOS calls it "Multi-Output
/// Device" by default, and anything the user renamed still tends to say so.
pub fn find_multi_output() -> Option<(DeviceId, String)> {
    outputs().into_iter().find(|(_, name)| is_multi_output(name))
}

pub fn is_multi_output(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("multi-output") || name.contains("multi output")
}

/// How many channels a device records, across all its input streams.
pub fn input_channels(device: DeviceId) -> u32 {
    let address = PropertyAddress {
        selector: kAudioDevicePropertyStreamConfiguration,
        scope: kAudioObjectPropertyScopeInput,
        element: kAudioObjectPropertyElementMain,
    };
    let mut size: u32 = 0;

    let status =
        unsafe { AudioObjectGetPropertyDataSize(device, &address, 0, std::ptr::null(), &mut size) };
    if status != 0 || size == 0 {
        return 0;
    }

    let mut buffer = vec![0u8; size as usize];
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            buffer.as_mut_ptr() as *mut c_void,
        )
    };
    if status != 0 {
        return 0;
    }

    channels_in(&buffer[..size as usize])
}

/// Sum the channels of an AudioBufferList: a count, padding to eight bytes,
/// then that many 16-byte buffers that each start with their channel count.
/// Only what the bytes actually hold is read.
fn channels_in(list: &[u8]) -> u32 {
    let read = |at: usize| list.get(at..at + 4).map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]));
    let count = read(0).unwrap_or(0) as usize;
    (0..count).map_while(|i| read(8 + i * 16)).sum()
}

/// A device's UID, which is how an AudioQueue is told which device to use.
/// The CFString it wraps is owned here and released when this is dropped.
pub struct DeviceUid(*const c_void);

impl DeviceUid {
    pub fn as_cf_string(&self) -> *const c_void {
        self.0
    }
}

impl Drop for DeviceUid {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

pub fn uid(device: DeviceId) -> Option<DeviceUid> {
    let address = PropertyAddress::global(kAudioDevicePropertyDeviceUID);
    let mut cf_string: *const c_void = std::ptr::null();
    let mut size = std::mem::size_of::<*const c_void>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut cf_string as *mut _ as *mut c_void,
        )
    };

    (status == 0 && !cf_string.is_null()).then_some(DeviceUid(cf_string))
}

pub(crate) fn all_devices() -> Vec<DeviceId> {
    let address = PropertyAddress::global(kAudioHardwarePropertyDevices);
    let mut size: u32 = 0;

    let status = unsafe {
        AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &address, 0, std::ptr::null(), &mut size)
    };
    if status != 0 || size == 0 {
        return Vec::new();
    }

    let count = size as usize / std::mem::size_of::<DeviceId>();
    let mut devices = vec![0 as DeviceId; count];

    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            devices.as_mut_ptr() as *mut c_void,
        )
    };

    if status == 0 {
        devices
    } else {
        Vec::new()
    }
}

/// A device with no output streams is a microphone, not somewhere to play.
fn has_output_channels(device: DeviceId) -> bool {
    let address = PropertyAddress {
        selector: kAudioDevicePropertyStreamConfiguration,
        scope: kAudioObjectPropertyScopeOutput,
        element: kAudioObjectPropertyElementMain,
    };
    let mut size: u32 = 0;

    let status =
        unsafe { AudioObjectGetPropertyDataSize(device, &address, 0, std::ptr::null(), &mut size) };
    if status != 0 || size == 0 {
        return false;
    }

    // AudioBufferList: a count followed by that many buffers. An empty list is
    // the count alone, which is what an input-only device reports.
    let mut buffer = vec![0u8; size as usize];
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            buffer.as_mut_ptr() as *mut c_void,
        )
    };
    if status != 0 || buffer.len() < 4 {
        return false;
    }

    u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) > 0
}

fn device_name(device: DeviceId) -> Option<String> {
    let address = PropertyAddress::global(kAudioObjectPropertyName);
    let mut cf_string: *const c_void = std::ptr::null();
    let mut size = std::mem::size_of::<*const c_void>() as u32;

    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut cf_string as *mut _ as *mut c_void,
        )
    };
    if status != 0 || cf_string.is_null() {
        return None;
    }

    let mut buffer = [0 as c_char; 256];
    let ok = unsafe {
        CFStringGetCString(cf_string, buffer.as_mut_ptr(), buffer.len() as isize, kCFStringEncodingUTF8)
    };
    unsafe { CFRelease(cf_string) };

    if ok == 0 {
        return None;
    }
    unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_str().ok().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_character_codes_match_coreaudio() {
        assert_eq!(kAudioHardwarePropertyDefaultOutputDevice, 0x644F_7574);
        assert_eq!(kAudioHardwarePropertyDevices, 0x6465_7623);
        assert_eq!(kAudioObjectPropertyScopeGlobal, 0x676C_6F62);
    }

    #[test]
    fn nominal_rate_code_matches_coreaudio() {
        assert_eq!(kAudioDevicePropertyNominalSampleRate, 0x6E73_7274); // 'nsrt'
    }

    #[test]
    fn input_property_codes_match_coreaudio() {
        assert_eq!(kAudioDevicePropertyDeviceUID, 0x7569_6420); // 'uid '
        assert_eq!(kAudioObjectPropertyScopeInput, 0x696E_7074); // 'inpt'
    }

    // An AudioBufferList as CoreAudio lays it out on a 64-bit Mac: the count,
    // four bytes of padding, then 16 bytes per buffer.
    #[test]
    fn counts_channels_across_every_input_stream() {
        let mut list = vec![0u8; 8 + 2 * 16];
        list[0..4].copy_from_slice(&2u32.to_ne_bytes());
        list[8..12].copy_from_slice(&2u32.to_ne_bytes());
        list[24..28].copy_from_slice(&1u32.to_ne_bytes());
        assert_eq!(channels_in(&list), 3);
    }

    #[test]
    fn a_list_shorter_than_it_claims_counts_what_is_there() {
        let mut list = vec![0u8; 8 + 16];
        list[0..4].copy_from_slice(&3u32.to_ne_bytes());
        list[8..12].copy_from_slice(&2u32.to_ne_bytes());
        assert_eq!(channels_in(&list), 2);
        assert_eq!(channels_in(&[]), 0);
    }

    #[test]
    fn recognises_a_multi_output_device_by_name() {
        assert!(is_multi_output("Multi-Output Device"));
        assert!(is_multi_output("My Multi Output"));
        assert!(!is_multi_output("MacBook Pro Speakers"));
        assert!(!is_multi_output("BlackHole 2ch"));
    }
}
