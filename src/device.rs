use crate::error::AppError;
use anyhow::Result;
use evdev::{Device, KeyCode};
use std::path::PathBuf;

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
            // Check for some common keyboard keys
            let keyboard_keys = [
                KeyCode::KEY_A,
                KeyCode::KEY_Z,
                KeyCode::KEY_SPACE,
                KeyCode::KEY_ENTER,
            ];

            for key in keyboard_keys.iter() {
                if keys.contains(*key) {
                    return true;
                }
            }
        }

        false
    }

    pub fn get_key_event(&mut self) -> Result<Option<(KeyCode, i32)>> {
        for event in self.device.fetch_events()? {
            if event.event_type() == evdev::EventType::KEY {
                return Ok(Some((KeyCode(event.code()), event.value())));
            }
        }

        Ok(None)
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
