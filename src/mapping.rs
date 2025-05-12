use crate::controller::VirtualController;
use crate::device::InputDevice;
use anyhow::Result;
use crossbeam_channel::{bounded, select, tick};
use evdev::Device;
use evdev::EventType;
use evdev::InputEvent;
use evdev::KeyCode;
use parking_lot::{Mutex, RwLock};
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct DeviceMapper {
    pub keyboard: InputDevice,
    pub controllers: Vec<VirtualController>,
    pub running: Arc<Mutex<bool>>,
    pub mapped_keys: Arc<RwLock<HashSet<KeyCode>>>,
}

impl DeviceMapper {
    pub fn new(keyboard: InputDevice) -> Self {
        DeviceMapper {
            keyboard,
            controllers: Vec::new(),
            running: Arc::new(Mutex::new(false)),
            mapped_keys: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub fn add_controller(&mut self, controller: VirtualController) {
        // Update mapped_keys set with the controller's key mappings
        let mut mapped_keys = self.mapped_keys.write();
        for key in controller.key_mapping.read().keys() {
            mapped_keys.insert(*key);
        }

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
            .map(|c| {
                let mapping = c.key_mapping.read().clone();
                (c.name.clone(), mapping)
            })
            .collect();

        *self.running.lock() = true;
        let running = self.running.clone();
        let mapped_keys_arc = self.mapped_keys.clone();

        // Create a signal channel
        let (signal_tx, signal_rx) = bounded(1);

        // Handle SIGINT, SIGTERM, and SIGHUP
        let signal_running = running.clone();
        let signal_tx_clone = signal_tx.clone();

        // Set up signal handler
        let _signal_thread = thread::spawn(move || {
            let mut signals = signal_hook::iterator::Signals::new(&[
                signal_hook::consts::SIGINT,
                signal_hook::consts::SIGTERM,
                signal_hook::consts::SIGHUP,
            ])
            .unwrap();

            for sig in signals.forever() {
                println!("Received signal {:?}, shutting down...", sig);
                *signal_running.lock() = false;
                let _ = signal_tx_clone.send(());
                break;
            }
        });

        let handle = thread::spawn(move || -> Result<()> {
            let mut keyboard = Device::open(&keyboard_path)?;

            // Get the mapped keys
            let mapped_keys = mapped_keys_arc.read().clone();

            // Create the controller devices
            let mut controllers = Vec::new();

            for (name, key_mapping) in controller_settings {
                let controller = VirtualController::new(&name)?;

                // Apply the key mappings
                let mut mapping = controller.key_mapping.write();
                for (key, target) in key_mapping {
                    mapping.insert(key, target);
                }
                drop(mapping); // Explicitly release the write lock

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

                    // Create a ticker for periodic checking (100ms)
                    let ticker = tick(Duration::from_millis(100));

                    // Main processing loop
                    while *running.lock() {
                        // Use crossbeam's select for efficient waiting
                        select! {
                            recv(signal_rx) -> _ => {
                                // Signal received, exit loop
                                println!("Signal received, exiting keyboard mapping");
                                break;
                            },
                            recv(ticker) -> _ => {
                                // Periodic check if we should keep running
                                if !*running.lock() {
                                    break;
                                }
                            },
                            default => {
                                // Process keyboard events
                                for ev in keyboard.fetch_events()? {
                                    if ev.event_type() == EventType::KEY {
                                        let key_code = KeyCode::new(ev.code());
                                        let value = ev.value();

                                        if mapped_keys.contains(&key_code) {
                                            for controller in &mut controllers {
                                                let target_key = {
                                                    let mapping = controller.key_mapping.read();
                                                    mapping.get(&key_code).copied()
                                                }; // Read lock is dropped here

                                                if let Some(target_key) = target_key {
                                                    controller.handle_key_event(target_key, value)?;
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
        *self.running.lock() = false;
        thread::sleep(Duration::from_millis(200));
    }

    pub fn capture_key(&mut self) -> Result<KeyCode> {
        // Wait for a key press from the keyboard
        println!("Press a key to capture mapping...");

        // Create a copy of the path for the capture thread
        let keyboard_path = self.keyboard.path.clone();

        // Use a channel to communicate between threads
        let (tx, rx) = crossbeam_channel::bounded(1);

        // Spawn a thread to capture the key
        let handle = thread::spawn(move || -> Result<()> {
            let mut keyboard = Device::open(&keyboard_path)?;

            loop {
                for event in keyboard.fetch_events()? {
                    if event.event_type() == EventType::KEY && event.value() == 1 {
                        // Key press (not release)
                        let key_code = KeyCode::new(event.code());
                        tx.send(key_code).unwrap();
                        return Ok(());
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });

        // Wait for the key to be captured
        let key = rx.recv()?;

        // Join the thread
        handle.join().expect("Failed to join key capture thread")?;

        Ok(key)
    }
}
