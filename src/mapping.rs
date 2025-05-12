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
        let keyboard_path = std::mem::take(&mut self.keyboard.path);
        let mut controllers = std::mem::take(&mut self.controllers);

        // Create a set of all mapped keys for quick lookup
        let mut mapped_keys = std::collections::HashSet::new();
        for controller in &controllers {
            for key in controller.key_mapping.keys() {
                mapped_keys.insert(*key);
            }
        }

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();

        // Create a signal channel that will be used to notify the thread about signals
        let (signal_tx, signal_rx) = std::sync::mpsc::channel();

        // Set up a single signal handler for all relevant signals
        let signal_running = running.clone();
        let signal_tx_clone = signal_tx.clone();

        // Handle SIGINT (Ctrl+C), SIGTERM, and SIGHUP with signal-hook
        let _signal_thread = thread::spawn(move || {
            let mut signals = signal_hook::iterator::Signals::new(&[
                signal_hook::consts::SIGINT,  // Ctrl+C
                signal_hook::consts::SIGTERM, // Termination signal
                signal_hook::consts::SIGHUP,  // Terminal closed
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
            let mut keyboard = Device::open(keyboard_path)?;

            // Grab the keyboard exclusively to intercept all events
            match keyboard.grab() {
                Ok(_) => {
                    println!("Keyboard grabbed successfully");

                    // Create the virtual keyboard for passing through non-mapped keys
                    let mut virtual_kbd = {
                        let supported_keys = keyboard.supported_keys().unwrap_or_default();
                        evdev::uinput::VirtualDevice::builder()?
                            .name("Forwarded Keyboard")
                            .with_keys(&supported_keys)?
                            .build()?
                    };

                    // Main processing loop with signal handling
                    while running.load(Ordering::SeqCst) {
                        // Check for signals with a short timeout to ensure responsive signal handling
                        match signal_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                            Ok(_) => {
                                // Signal received, exit loop
                                println!("Signal received, exiting keyboard mapping");
                                break;
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                // No signal, continue processing
                            }
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
