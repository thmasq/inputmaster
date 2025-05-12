use crate::controller::VirtualController;
use crate::device::InputDevice;
use anyhow::Result;
use evdev::Device;
use evdev::EventType;
use evdev::InputEvent;
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
        // Check if we have any controllers
        if self.controllers.is_empty() {
            return Err(anyhow::anyhow!("No controllers available to map"));
        }

        // Store the path to the keyboard
        let keyboard_path = self.keyboard.path.clone();

        // Store the key mappings and device names we need to recreate
        let controller_settings: Vec<_> = self
            .controllers
            .iter()
            .map(|c| (c.name.clone(), c.key_mapping.clone()))
            .collect();

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();

        // Create a signal channel
        let (signal_tx, signal_rx) = std::sync::mpsc::channel();

        // Set up signal handler
        let signal_running = running.clone();
        let signal_tx_clone = signal_tx.clone();

        // Handle SIGINT, SIGTERM, and SIGHUP
        let _signal_thread = thread::spawn(move || {
            let mut signals = signal_hook::iterator::Signals::new(&[
                signal_hook::consts::SIGINT,
                signal_hook::consts::SIGTERM,
                signal_hook::consts::SIGHUP,
            ])
            .unwrap();

            for sig in signals.forever() {
                println!("Received signal {:?}, shutting down...", sig);
                signal_running.store(false, Ordering::SeqCst);
                let _ = signal_tx_clone.send(());
                break;
            }
        });

        let handle = thread::spawn(move || -> Result<()> {
            let mut keyboard = Device::open(&keyboard_path)?;

            // Create a set of all mapped keys for quick lookup
            let mut mapped_keys = std::collections::HashSet::new();

            // Create the controller devices
            let mut controllers = Vec::new();

            for (name, key_mapping) in controller_settings {
                let mut controller = VirtualController::new(&name)?;

                // Apply the key mappings
                for (key, target) in key_mapping {
                    controller.key_mapping.insert(key, target);
                    mapped_keys.insert(key);
                }

                controllers.push(controller);
            }

            // Grab the keyboard exclusively
            match keyboard.grab() {
                Ok(_) => {
                    println!("Keyboard grabbed successfully");

                    // Create virtual keyboard for passing through non-mapped keys
                    let mut virtual_kbd = {
                        let supported_keys = keyboard.supported_keys().unwrap_or_default();
                        evdev::uinput::VirtualDevice::builder()?
                            .name("Forwarded Keyboard")
                            .with_keys(&supported_keys)?
                            .build()?
                    };

                    // Main processing loop
                    while running.load(Ordering::SeqCst) {
                        // Check for signals
                        match signal_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                            Ok(_) => {
                                // Signal received, exit loop
                                println!("Signal received, exiting keyboard mapping");
                                break;
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                            Err(_) => {
                                // Channel error, exit loop
                                eprintln!("Signal channel error, exiting keyboard mapping");
                                break;
                            }
                        }

                        // Process keyboard events
                        for ev in keyboard.fetch_events()? {
                            if ev.event_type() == EventType::KEY {
                                let key_code = KeyCode::new(ev.code());
                                let value = ev.value();

                                if mapped_keys.contains(&key_code) {
                                    for controller in &mut controllers {
                                        if controller.key_mapping.contains_key(&key_code) {
                                            controller.handle_key_event(key_code, value)?;
                                        }
                                    }
                                } else {
                                    // Forward to virtual keyboard
                                    let events =
                                        [InputEvent::new(EventType::KEY.0, key_code.0, value)];
                                    virtual_kbd.emit(&events)?;
                                }
                            } else {
                                // Forward non-key events
                                let events = [ev];
                                virtual_kbd.emit(&events)?;
                            }
                        }
                    }

                    // Always ungrab the keyboard before exiting
                    match keyboard.ungrab() {
                        Ok(_) => println!("Keyboard released successfully"),
                        Err(e) => eprintln!("Error releasing keyboard: {}", e),
                    }
                }
                Err(e) => {
                    eprintln!("Failed to grab keyboard: {}", e);
                    return Err(anyhow::anyhow!("Failed to grab keyboard: {}", e));
                }
            }

            Ok(())
        });

        Ok(handle)
    }

    pub fn stop_mapping(&self) {
        self.running.store(false, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(200));
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
