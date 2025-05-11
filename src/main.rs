mod controller;
mod device;
mod error;
mod mapping;
mod ui;

use anyhow::Result;
use std::process;
use std::sync::mpsc;

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

                    // Setup Ctrl+C handler
                    let (tx, rx) = mpsc::channel();
                    let tx_clone = tx.clone();

                    ctrlc::set_handler(move || {
                        let _ = tx_clone.send(());
                    })?;

                    // Start the mapping thread
                    mapping_thread = Some(mapper.start_mapping()?);

                    // Wait for Ctrl+C
                    rx.recv()?;
                    mapper.stop_mapping();

                    if let Some(handle) = mapping_thread.take() {
                        handle.join().expect("Failed to join mapping thread")?;
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

    if let Some(handle) = mapping_thread {
        mapper.stop_mapping();
        handle.join().expect("Failed to join mapping thread")?;
    }

    ui.cleanup()?;

    Ok(())
}
