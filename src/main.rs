mod hid;
mod tui;

use std::{path::PathBuf, process};

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    // 1. Run the TUI device picker
    let selected: Option<PathBuf> = tui::pick_device()?;

    let Some(input_path) = selected else {
        println!("No device selected. Exiting.");
        return Ok(());
    };

    println!("Selected input device: {}", input_path.display());
    println!("Starting HID forwarding. Press Ctrl+C to stop.");

    // 2. Start forwarding events from evdev to the HID gadget
    hid::run_forwarder(&input_path)?;

    Ok(())
}
