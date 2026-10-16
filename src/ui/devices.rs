//! System device enumeration: audio outputs, audio inputs, MIDI inputs.
//!
//! These are cached once at startup so the UI never queries the OS on the
//! render path.

use cpal::traits::{DeviceTrait, HostTrait};

pub fn computer_output_names() -> Vec<String> {
    let host = cpal::default_host();
    let mut names: Vec<String> = host
        .output_devices()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|device| device.name().ok())
        .collect();
    names.sort();
    names.dedup();
    names
}

pub fn computer_input_names() -> Vec<String> {
    let host = cpal::default_host();
    let mut names: Vec<String> = host
        .input_devices()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|device| device.name().ok())
        .collect();
    names.sort();
    names.dedup();
    names
}

pub fn midi_input_names() -> Vec<String> {
    let Ok(client) = midir::MidiInput::new("miniunicorn-device-list") else {
        return Vec::new();
    };
    let mut names: Vec<String> = client
        .ports()
        .iter()
        .filter_map(|port| client.port_name(port).ok())
        .collect();
    names.sort();
    names.dedup();
    names
}
