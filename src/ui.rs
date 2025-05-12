use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode as CtKeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};
use std::io::{Write, stdout};

use crate::controller::VirtualController;
use crate::device::InputDevice;
use crate::mapping::DeviceMapper;

pub struct UI {
    stdout: std::io::Stdout,
}

impl UI {
    pub fn new() -> Self {
        UI { stdout: stdout() }
    }

    pub fn init(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(
            self.stdout,
            Clear(ClearType::All),
            Hide,
            MoveTo(0, 0),
            SetForegroundColor(Color::Cyan),
            Print("Keyboard to Controller Mapper"),
            ResetColor
        )?;

        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.stdout, Clear(ClearType::All), Show, MoveTo(0, 0))?;

        Ok(())
    }

    pub fn show_devices(&mut self, devices: &[InputDevice]) -> Result<()> {
        execute!(
            self.stdout,
            MoveTo(0, 2),
            SetForegroundColor(Color::Yellow),
            Print("Available Keyboard Devices:"),
            ResetColor,
            MoveTo(0, 3)
        )?;

        for (i, device) in devices.iter().enumerate() {
            execute!(
                self.stdout,
                MoveTo(2, 5 + i as u16),
                Print(format!(
                    "{}. {} ({})",
                    i + 1,
                    device.name,
                    device.path.display()
                ))
            )?;
        }

        execute!(
            self.stdout,
            MoveTo(0, 5 + devices.len() as u16 + 2),
            Print("Select a keyboard device (1-"),
            Print(devices.len()),
            Print("): ")
        )?;

        Ok(())
    }

    pub fn select_device(&mut self, devices: &[InputDevice]) -> Result<usize> {
        loop {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if let CtKeyCode::Char(c) = code {
                    if let Some(idx) = c.to_digit(10) {
                        let idx = idx as usize;
                        if idx >= 1 && idx <= devices.len() {
                            return Ok(idx - 1);
                        }
                    }
                }
            }
        }
    }

    pub fn prompt_yes_no(&mut self, question: &str) -> Result<bool> {
        execute!(
            self.stdout,
            Clear(ClearType::All),
            MoveTo(2, 2),
            Print(format!("{} (y/n): ", question))
        )?;

        loop {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    CtKeyCode::Char('y') | CtKeyCode::Char('Y') => return Ok(true),
                    CtKeyCode::Char('n') | CtKeyCode::Char('N') => return Ok(false),
                    _ => { /* Ignore other keys */ }
                }
            }
        }
    }

    pub fn map_controller_buttons(
        &mut self,
        controller: &mut VirtualController,
        mapper: &mut DeviceMapper,
    ) -> Result<()> {
        execute!(
            self.stdout,
            Clear(ClearType::All),
            MoveTo(0, 0),
            SetForegroundColor(Color::Green),
            Print(format!("Mapping for {}", controller.name)),
            ResetColor,
            MoveTo(0, 2),
            Print("Press keyboard keys to map to the following controller buttons:\n"),
            Print("(Press the key on your keyboard when prompted)")
        )?;

        let buttons_to_map = VirtualController::get_available_button_mappings();

        // Temporarily disable raw mode to allow direct evdev input
        disable_raw_mode()?;

        for (i, (button_code, button_name)) in buttons_to_map.iter().enumerate() {
            // Clear the line
            execute!(
                self.stdout,
                MoveTo(2, 5 + i as u16),
                Clear(ClearType::CurrentLine),
                Print(format!("Press a key to map to {}: ", button_name))
            )?;

            self.stdout.flush()?;

            // Capture key press from the keyboard
            let key_code = mapper.capture_key()?;

            execute!(
                self.stdout,
                MoveTo(40, 5 + i as u16),
                Print(format!("Mapped to {:?}", key_code))
            )?;

            // Add the mapping
            controller.key_mapping.insert(key_code, *button_code);
        }

        // Re-enable raw mode for the UI
        enable_raw_mode()?;

        execute!(
            self.stdout,
            MoveTo(2, 5 + buttons_to_map.len() as u16 + 2),
            Print("Mapping complete! Press any key to continue.")
        )?;

        // Wait for a key press
        event::read()?;

        Ok(())
    }

    pub fn show_main_menu(&mut self) -> Result<u8> {
        execute!(
            self.stdout,
            Clear(ClearType::All),
            MoveTo(2, 2),
            SetForegroundColor(Color::Cyan),
            Print("Keyboard to Controller Mapper"),
            ResetColor,
            MoveTo(2, 4),
            Print("1. Create a new controller"),
            MoveTo(2, 5),
            Print("2. List active controllers"),
            MoveTo(2, 6),
            Print("3. Start mapping (begin using controllers)"),
            MoveTo(2, 7),
            Print("4. Exit"),
            MoveTo(2, 9),
            Print("Select an option (1-4): ")
        )?;

        loop {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if let CtKeyCode::Char(c) = code {
                    if let Some(option) = c.to_digit(10) {
                        if option >= 1 && option <= 4 {
                            return Ok(option as u8);
                        }
                    }
                }
            }
        }
    }

    pub fn list_controllers(&mut self, controllers: &[VirtualController]) -> Result<()> {
        execute!(
            self.stdout,
            Clear(ClearType::All),
            MoveTo(2, 2),
            SetForegroundColor(Color::Yellow),
            Print("Active Controllers:"),
            ResetColor
        )?;

        if controllers.is_empty() {
            execute!(
                self.stdout,
                MoveTo(2, 4),
                Print("No controllers created yet.")
            )?;
        } else {
            for (i, controller) in controllers.iter().enumerate() {
                execute!(
                    self.stdout,
                    MoveTo(2, 4 + i as u16),
                    Print(format!(
                        "{}. {} ({} keys mapped)",
                        i + 1,
                        controller.name,
                        controller.key_mapping.len()
                    ))
                )?;
            }
        }

        execute!(
            self.stdout,
            MoveTo(2, 6 + controllers.len() as u16),
            Print("Press any key to continue...")
        )?;

        event::read()?;

        Ok(())
    }

    pub fn show_mapping_active(&mut self) -> Result<()> {
        execute!(
            self.stdout,
            Clear(ClearType::All),
            MoveTo(2, 2),
            SetForegroundColor(Color::Green),
            Print("Mapping Active!"),
            ResetColor,
            MoveTo(2, 4),
            Print("Your keyboard inputs are now being sent to the virtual controllers."),
            MoveTo(2, 6),
            Print("Press Delete to stop and return to the menu.")
        )?;

        self.stdout.flush()?;

        Ok(())
    }
}
