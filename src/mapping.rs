use crate::controller::VirtualController;
use crate::device::InputDevice;
use anyhow::Result;
use evdev::Device;
use evdev::EventType;
use evdev::KeyCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub struct DeviceMapper {
    pub keyboard: InputDevice,
    pub controllers: Vec<VirtualController>,
    pub running: Arc<AtomicBool>,
}

impl DeviceMapper {
    pub fn new(keyboard: InputDevice) -> Self {
        DeviceMapper {
            keyboard,
            controllers: Vec::new(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn add_controller(&mut self, controller: VirtualController) {
        self.controllers.push(controller);
    }

    pub fn start_mapping(&mut self) -> Result<thread::JoinHandle<Result<()>>> {
        let keyboard_path = std::mem::take(&mut self.keyboard.path);
        let mut controllers = std::mem::take(&mut self.controllers);

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();

        let handle = thread::spawn(move || -> Result<()> {
            let mut keyboard = Device::open(keyboard_path)?;

            while running.load(Ordering::SeqCst) {
                for ev in keyboard.fetch_events()? {
                    if ev.event_type() == EventType::KEY {
                        let key_code = KeyCode::new(ev.code());
                        let value = ev.value();
                        for controller in &mut controllers {
                            controller.handle_key_event(key_code, value)?;
                        }
                    }
                }
                thread::sleep(Duration::from_millis(5));
            }

            Ok(())
        });

        Ok(handle)
    }

    pub fn stop_mapping(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn capture_key(&mut self) -> Result<KeyCode> {
        // Wait for a key press from the keyboard
        println!("Press a key to capture mapping...");

        loop {
            if let Ok(Some((key_code, value))) = self.keyboard.get_key_event() {
                if value == 1 {
                    // Key press (not release)
                    return Ok(key_code);
                }
            }

            thread::sleep(Duration::from_millis(10));
        }
    }
}
