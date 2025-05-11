use anyhow::Result;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, uinput::VirtualDevice};
use std::collections::HashMap;

pub struct VirtualController {
    pub device: VirtualDevice,
    pub name: String,
    pub key_mapping: HashMap<KeyCode, KeyCode>,
}

impl VirtualController {
    pub fn new(name: &str) -> Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();

        keys.insert(KeyCode::BTN_SOUTH); // A
        keys.insert(KeyCode::BTN_EAST); // B
        keys.insert(KeyCode::BTN_NORTH); // X
        keys.insert(KeyCode::BTN_WEST); // Y
        keys.insert(KeyCode::BTN_TL); // Left Shoulder
        keys.insert(KeyCode::BTN_TR); // Right Shoulder
        keys.insert(KeyCode::BTN_SELECT); // Back
        keys.insert(KeyCode::BTN_START); // Start
        keys.insert(KeyCode::BTN_MODE); // Guide
        keys.insert(KeyCode::BTN_THUMBL); // Left Thumb
        keys.insert(KeyCode::BTN_THUMBR); // Right Thumb

        keys.insert(KeyCode::BTN_DPAD_UP);
        keys.insert(KeyCode::BTN_DPAD_DOWN);
        keys.insert(KeyCode::BTN_DPAD_LEFT);
        keys.insert(KeyCode::BTN_DPAD_RIGHT);

        let device = VirtualDevice::builder()?
            .name(name)
            .with_keys(&keys)?
            .build()?;

        Ok(VirtualController {
            device,
            name: name.to_string(),
            key_mapping: HashMap::new(),
        })
    }

    pub fn apply_default_mapping(&mut self) {
        self.key_mapping = HashMap::new();

        // WASD for D-pad
        self.key_mapping
            .insert(KeyCode::KEY_W, KeyCode::BTN_DPAD_UP);
        self.key_mapping
            .insert(KeyCode::KEY_S, KeyCode::BTN_DPAD_DOWN);
        self.key_mapping
            .insert(KeyCode::KEY_A, KeyCode::BTN_DPAD_LEFT);
        self.key_mapping
            .insert(KeyCode::KEY_D, KeyCode::BTN_DPAD_RIGHT);

        // Arrow keys also for D-pad
        self.key_mapping
            .insert(KeyCode::KEY_UP, KeyCode::BTN_DPAD_UP);
        self.key_mapping
            .insert(KeyCode::KEY_DOWN, KeyCode::BTN_DPAD_DOWN);
        self.key_mapping
            .insert(KeyCode::KEY_LEFT, KeyCode::BTN_DPAD_LEFT);
        self.key_mapping
            .insert(KeyCode::KEY_RIGHT, KeyCode::BTN_DPAD_RIGHT);

        // Face buttons
        self.key_mapping.insert(KeyCode::KEY_J, KeyCode::BTN_SOUTH); // A
        self.key_mapping.insert(KeyCode::KEY_K, KeyCode::BTN_EAST); // B
        self.key_mapping.insert(KeyCode::KEY_U, KeyCode::BTN_NORTH); // X
        self.key_mapping.insert(KeyCode::KEY_I, KeyCode::BTN_WEST); // Y

        // Shoulders
        self.key_mapping.insert(KeyCode::KEY_Q, KeyCode::BTN_TL); // Left Shoulder
        self.key_mapping.insert(KeyCode::KEY_E, KeyCode::BTN_TR); // Right Shoulder

        // Special buttons
        self.key_mapping
            .insert(KeyCode::KEY_TAB, KeyCode::BTN_SELECT); // Back
        self.key_mapping
            .insert(KeyCode::KEY_ENTER, KeyCode::BTN_START); // Start
        self.key_mapping
            .insert(KeyCode::KEY_SPACE, KeyCode::BTN_MODE); // Guide
    }

    pub fn handle_key_event(&mut self, key_code: KeyCode, value: i32) -> Result<()> {
        if let Some(&controller_key) = self.key_mapping.get(&key_code) {
            let events = [InputEvent::new(EventType::KEY.0, controller_key.0, value)];

            self.device.emit(&events)?;
        }

        Ok(())
    }

    pub fn get_available_button_mappings() -> Vec<(KeyCode, &'static str)> {
        vec![
            (KeyCode::BTN_SOUTH, "A Button"),
            (KeyCode::BTN_EAST, "B Button"),
            (KeyCode::BTN_NORTH, "X Button"),
            (KeyCode::BTN_WEST, "Y Button"),
            (KeyCode::BTN_TL, "Left Shoulder"),
            (KeyCode::BTN_TR, "Right Shoulder"),
            (KeyCode::BTN_SELECT, "Select Button"),
            (KeyCode::BTN_START, "Start Button"),
            (KeyCode::BTN_MODE, "Guide Button"),
            (KeyCode::BTN_THUMBL, "Left Thumb"),
            (KeyCode::BTN_THUMBR, "Right Thumb"),
            (KeyCode::BTN_DPAD_UP, "D-Pad Up"),
            (KeyCode::BTN_DPAD_DOWN, "D-Pad Down"),
            (KeyCode::BTN_DPAD_LEFT, "D-Pad Left"),
            (KeyCode::BTN_DPAD_RIGHT, "D-Pad Right"),
        ]
    }
}
