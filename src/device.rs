use crate::error::AppError;
use anyhow::Result;
use evdev::{Device, KeyCode};
use std::path::PathBuf;

#[allow(dead_code)]
pub struct InputDevice {
    pub path: PathBuf,
    pub device: Device,
    pub name: String,
    pub is_keyboard: bool,
}

impl InputDevice {
    pub fn new(path: PathBuf, device: Device) -> Self {
        let name = device.name().unwrap_or("Unknown device").to_string();
        let is_keyboard = Self::is_keyboard(&device);

        InputDevice {
            path,
            device,
            name,
            is_keyboard,
        }
    }

    fn is_keyboard(device: &Device) -> bool {
        // Check if this device has keys that are typical for keyboards
        if let Some(keys) = device.supported_keys() {
            let essential_modifiers = [
                KeyCode::KEY_LEFTCTRL,  // Left Ctrl
                KeyCode::KEY_LEFTSHIFT, // Left Shift
                KeyCode::KEY_LEFTALT,   // Left Alt
            ];

            // Count how many essential modifiers this device has
            let mut modifier_count = 0;
            for key in essential_modifiers.iter() {
                if keys.contains(*key) {
                    modifier_count += 1;
                }
            }

            // If the device is missing any of these modifiers, it's not a main keyboard
            if modifier_count < essential_modifiers.len() {
                return false;
            }

            return true;
        }

        false
    }
}

pub fn discover_keyboards() -> Result<Vec<InputDevice>> {
    let mut keyboards = Vec::new();

    for (path, device) in evdev::enumerate() {
        let input_device = InputDevice::new(path, device);

        if input_device.is_keyboard {
            keyboards.push(input_device);
        }
    }

    if keyboards.is_empty() {
        return Err(AppError::NoKeyboardsFound.into());
    }

    Ok(keyboards)
}
