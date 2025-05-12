mod controller;
mod device;
mod error;
mod mapping;
mod ui;

use anyhow::Result;
use crossbeam_channel::bounded;
use std::process;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    let mut ui = ui::UI::new();
    ui.init()?;

    let keyboards = match device::discover_keyboards() {
        Ok(keyboards) => keyboards,
        Err(e) => {
            ui.cleanup()?;
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    ui.show_devices(&keyboards)?;
    let selected_idx = ui.select_device(&keyboards)?;
    let selected_keyboard = keyboards
        .into_iter()
        .nth(selected_idx)
        .expect("Selected keyboard not found");

    let mut mapper = mapping::DeviceMapper::new(selected_keyboard);

    let mut mapping_thread = None;

    'main_loop: loop {
        match ui.show_main_menu()? {
            1 => {
                // Create a new controller
                let controller_num = mapper.controllers.len() + 1;
                let controller_name = format!("Controller {}", controller_num);

                match controller::VirtualController::new(&controller_name) {
                    Ok(mut controller) => {
                        if controller_num == 1 {
                            // First controller gets default mapping
                            controller.apply_default_mapping();
                            ui.prompt_yes_no("First controller auto-mapped with default settings. Press any key to continue")?;
                        } else {
                            // Manual mapping for additional controllers
                            ui.map_controller_buttons(&mut controller, &mut mapper)?;
                        }

                        mapper.add_controller(controller);
                    }
                    Err(e) => {
                        eprintln!("Failed to create controller: {}", e);
                        ui.prompt_yes_no(&format!(
                            "Failed to create controller: {}. Continue?",
                            e
                        ))?;
                    }
                }
            }
            2 => {
                // List active controllers
                ui.list_controllers(&mapper.controllers)?;
            }
            3 => {
                // Start mapping
                if mapper.controllers.is_empty() {
                    ui.prompt_yes_no("No controllers created yet. Create one first?")?;
                    continue;
                }

                // Start the mapping in a background thread
                if mapping_thread.is_none() {
                    ui.show_mapping_active()?;

                    // Set up a way to detect when the user wants to stop mapping
                    let (stop_tx, stop_rx) = bounded(1);

                    // Create a separate thread to watch for user input to stop mapping
                    let running = Arc::new(parking_lot::Mutex::new(true));
                    let ui_running = running.clone();

                    // This thread will watch for Delete key press from the UI
                    let ui_thread = thread::spawn(move || {
                        while *ui_running.lock() {
                            // Check for Delete key press to quit
                            if let Ok(crossterm::event::Event::Key(key)) =
                                crossterm::event::poll(Duration::from_millis(100))
                                    .and_then(|_| crossterm::event::read())
                            {
                                if let crossterm::event::KeyCode::Delete = key.code {
                                    let _ = stop_tx.send(());
                                    break;
                                }
                            }
                        }
                    });

                    // Start the mapping thread
                    match mapper.start_mapping() {
                        Ok(thread_handle) => {
                            mapping_thread = Some(thread_handle);

                            // Wait for signal from UI thread (user pressed Delete key)
                            stop_rx.recv()?;

                            // Stop the mapping process
                            *running.lock() = false;
                            mapper.stop_mapping();

                            // Take the mapping thread out of the Option and join it
                            if let Some(handle) = mapping_thread.take() {
                                handle.join().expect("Failed to join mapping thread")?;
                            }

                            ui_thread.join().expect("Failed to join UI thread");
                        }
                        Err(e) => {
                            ui.prompt_yes_no(&format!(
                                "Failed to start mapping: {}. Continue?",
                                e
                            ))?;
                        }
                    }
                }
            }
            4 => {
                // Exit
                break 'main_loop;
            }
            _ => unreachable!(),
        }
    }

    if let Some(handle) = mapping_thread.take() {
        mapper.stop_mapping();
        handle.join().expect("Failed to join mapping thread")?;
    }

    ui.cleanup()?;

    Ok(())
}
