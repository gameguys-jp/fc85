//! libretro の API の、このコアが使う部分。libretro.h（MIT、libretro の API の定義）の形と値に合わせて
//! 手で書いた（生成の道具を足さないため）。

#![allow(dead_code)]

use std::ffi::{c_char, c_int, c_uint, c_void};

pub const RETRO_API_VERSION: c_uint = 1;

pub const RETRO_DEVICE_JOYPAD: c_uint = 1;

pub const RETRO_DEVICE_ID_JOYPAD_B: c_uint = 0;
pub const RETRO_DEVICE_ID_JOYPAD_Y: c_uint = 1;
pub const RETRO_DEVICE_ID_JOYPAD_SELECT: c_uint = 2;
pub const RETRO_DEVICE_ID_JOYPAD_START: c_uint = 3;
pub const RETRO_DEVICE_ID_JOYPAD_UP: c_uint = 4;
pub const RETRO_DEVICE_ID_JOYPAD_DOWN: c_uint = 5;
pub const RETRO_DEVICE_ID_JOYPAD_LEFT: c_uint = 6;
pub const RETRO_DEVICE_ID_JOYPAD_RIGHT: c_uint = 7;
pub const RETRO_DEVICE_ID_JOYPAD_A: c_uint = 8;
pub const RETRO_DEVICE_ID_JOYPAD_X: c_uint = 9;

pub const RETRO_REGION_NTSC: c_uint = 0;

pub const RETRO_MEMORY_SAVE_RAM: c_uint = 0;
pub const RETRO_MEMORY_SYSTEM_RAM: c_uint = 2;

pub const RETRO_ENVIRONMENT_SET_MESSAGE: c_uint = 6;
pub const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: c_uint = 9;
pub const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: c_uint = 10;
pub const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: c_uint = 11;
pub const RETRO_ENVIRONMENT_GET_VARIABLE: c_uint = 15;
pub const RETRO_ENVIRONMENT_SET_VARIABLES: c_uint = 16;
pub const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: c_uint = 17;
pub const RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME: c_uint = 18;
pub const RETRO_ENVIRONMENT_GET_LOG_INTERFACE: c_uint = 27;
pub const RETRO_ENVIRONMENT_SET_GEOMETRY: c_uint = 37;
pub const RETRO_ENVIRONMENT_EXPERIMENTAL: c_uint = 0x10000;
pub const RETRO_ENVIRONMENT_GET_MIDI_INTERFACE: c_uint = 48 | RETRO_ENVIRONMENT_EXPERIMENTAL;
pub const RETRO_ENVIRONMENT_GET_MICROPHONE_INTERFACE: c_uint = 75 | RETRO_ENVIRONMENT_EXPERIMENTAL;
pub const RETRO_MICROPHONE_INTERFACE_VERSION: c_uint = 1;

pub const RETRO_PIXEL_FORMAT_XRGB8888: c_uint = 1;

pub const RETRO_LOG_DEBUG: c_uint = 0;
pub const RETRO_LOG_INFO: c_uint = 1;
pub const RETRO_LOG_WARN: c_uint = 2;
pub const RETRO_LOG_ERROR: c_uint = 3;

pub type EnvironmentFn = unsafe extern "C" fn(cmd: c_uint, data: *mut c_void) -> bool;
pub type VideoRefreshFn =
    unsafe extern "C" fn(data: *const c_void, width: c_uint, height: c_uint, pitch: usize);
pub type AudioSampleFn = unsafe extern "C" fn(left: i16, right: i16);
pub type MidiEnabledFn = unsafe extern "C" fn() -> bool;
pub type MidiReadFn = unsafe extern "C" fn(byte: *mut u8) -> bool;
pub type MidiWriteFn = unsafe extern "C" fn(byte: u8, delta_time: u32) -> bool;
pub type MidiFlushFn = unsafe extern "C" fn() -> bool;

/// `retro_midi_interface`（RETRO_ENVIRONMENT_GET_MIDI_INTERFACE）。write の delta_time は前の write からの µs。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetroMidiInterface {
    pub input_enabled: Option<MidiEnabledFn>,
    pub output_enabled: Option<MidiEnabledFn>,
    pub read: Option<MidiReadFn>,
    pub write: Option<MidiWriteFn>,
    pub flush: Option<MidiFlushFn>,
}
/// `retro_microphone_params_t`。rate は頼む標本の周波数（0 ならフロントエンドの既定）。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetroMicrophoneParams {
    pub rate: c_uint,
}
pub type OpenMicFn = unsafe extern "C" fn(params: *const RetroMicrophoneParams) -> *mut c_void;
pub type CloseMicFn = unsafe extern "C" fn(mic: *mut c_void);
pub type GetMicParamsFn =
    unsafe extern "C" fn(mic: *const c_void, params: *mut RetroMicrophoneParams) -> bool;
pub type SetMicStateFn = unsafe extern "C" fn(mic: *mut c_void, state: bool) -> bool;
pub type GetMicStateFn = unsafe extern "C" fn(mic: *const c_void) -> bool;
pub type ReadMicFn =
    unsafe extern "C" fn(mic: *mut c_void, samples: *mut i16, num_samples: usize) -> c_int;

/// `retro_microphone_interface`（RETRO_ENVIRONMENT_GET_MICROPHONE_INTERFACE）。interface_version はコアが
/// 頼む版を入れて渡す。マイクはモノラルの 16 bit で、開いた直後は止まっている（set_mic_state で動かす）。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetroMicrophoneInterface {
    pub interface_version: c_uint,
    pub open_mic: Option<OpenMicFn>,
    pub close_mic: Option<CloseMicFn>,
    pub get_params: Option<GetMicParamsFn>,
    pub set_mic_state: Option<SetMicStateFn>,
    pub get_mic_state: Option<GetMicStateFn>,
    pub read_mic: Option<ReadMicFn>,
}
pub type AudioSampleBatchFn = unsafe extern "C" fn(data: *const i16, frames: usize) -> usize;
pub type InputPollFn = unsafe extern "C" fn();
pub type InputStateFn =
    unsafe extern "C" fn(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16;
pub type LogPrintfFn = unsafe extern "C" fn(level: c_uint, fmt: *const c_char, ...);

#[repr(C)]
pub struct RetroLogCallback {
    pub log: Option<LogPrintfFn>,
}

#[repr(C)]
pub struct RetroSystemInfo {
    pub library_name: *const c_char,
    pub library_version: *const c_char,
    pub valid_extensions: *const c_char,
    pub need_fullpath: bool,
    pub block_extract: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetroGameGeometry {
    pub base_width: c_uint,
    pub base_height: c_uint,
    pub max_width: c_uint,
    pub max_height: c_uint,
    pub aspect_ratio: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetroSystemTiming {
    pub fps: f64,
    pub sample_rate: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetroSystemAvInfo {
    pub geometry: RetroGameGeometry,
    pub timing: RetroSystemTiming,
}

#[repr(C)]
pub struct RetroGameInfo {
    pub path: *const c_char,
    pub data: *const c_void,
    pub size: usize,
    pub meta: *const c_char,
}

#[repr(C)]
pub struct RetroVariable {
    pub key: *const c_char,
    pub value: *const c_char,
}

#[repr(C)]
pub struct RetroMessage {
    pub msg: *const c_char,
    pub frames: c_uint,
}

#[repr(C)]
pub struct RetroInputDescriptor {
    pub port: c_uint,
    pub device: c_uint,
    pub index: c_uint,
    pub id: c_uint,
    pub description: *const c_char,
}
