use std::{fs::OpenOptions, io::Write, path::Path};

use anyhow::{Context, Result};
use evdev::{Device, EventSummary, KeyCode, RelativeAxisCode, SynchronizationCode};

// Path to the HID gadget device (mouse).
pub const HID_DEVICE_PATH: &str = "/dev/hidg1";

// Simple 5-byte mouse report:
// [buttons, x, y, wheel, hwheel]
#[derive(Default)]
struct Report {
    buttons: u8,
    x: i8,
    y: i8,
    wheel: i8,
    hwheel: i8,
}

impl Report {
    #[inline]
    fn to_bytes(&self) -> [u8; 5] {
        [
            self.buttons,
            self.x as u8,
            self.y as u8,
            self.wheel as u8,
            self.hwheel as u8,
        ]
    }

    // After each SYN_REPORT we reset relative fields.
    #[inline]
    fn reset_motion(&mut self) {
        self.x = 0;
        self.y = 0;
        self.wheel = 0;
        self.hwheel = 0;
    }
}

#[inline]
fn clamp_i8(value: i32) -> i8 {
    value.clamp(i8::MIN as i32, i8::MAX as i32) as i8
}

#[inline]
fn update_button(byte: &mut u8, pressed: bool, mask: u8) {
    if pressed {
        *byte |= mask;
    } else {
        *byte &= !mask;
    }
}

/* Run the main forwarding loop:
 * - read events from the selected evdev device
 * - convert them into HID mouse reports
 * - write reports to /dev/hidg1
 */
pub fn run_forwarder(input_device: &Path) -> Result<()> {
    let mut device = Device::open(input_device)
        .with_context(|| format!("Failed to open input device {}", input_device.display()))?;

    // Grab the device so events are consumed only by us.
    device
        .grab()
        .with_context(|| "Failed to grab input device (try running as root)".to_string())?;

    let mut hid = OpenOptions::new()
        .write(true)
        .open(HID_DEVICE_PATH)
        .with_context(|| format!("Failed to open HID gadget at {HID_DEVICE_PATH}"))?;

    let mut report = Report::default();

    loop {
        for event in device
            .fetch_events()
            .context("Failed to read input events")?
        {
            match event.destructure() {
                EventSummary::RelativeAxis(_, code, value) => match code {
                    RelativeAxisCode::REL_X => report.x = clamp_i8(value),
                    RelativeAxisCode::REL_Y => report.y = clamp_i8(value),
                    RelativeAxisCode::REL_WHEEL => report.wheel = clamp_i8(value),
                    RelativeAxisCode::REL_HWHEEL => report.hwheel = clamp_i8(value),
                    _ => {}
                },

                EventSummary::Key(_, key, value) => {
                    let pressed = value == 1;
                    match key {
                        KeyCode::BTN_LEFT => update_button(&mut report.buttons, pressed, 0x01),
                        KeyCode::BTN_RIGHT => update_button(&mut report.buttons, pressed, 0x02),
                        KeyCode::BTN_MIDDLE => update_button(&mut report.buttons, pressed, 0x04),
                        KeyCode::BTN_SIDE | KeyCode::BTN_BACK => {
                            update_button(&mut report.buttons, pressed, 0x08)
                        }
                        KeyCode::BTN_EXTRA | KeyCode::BTN_FORWARD => {
                            update_button(&mut report.buttons, pressed, 0x10)
                        }
                        _ => {}
                    }
                }

                EventSummary::Synchronization(_, sync, _)
                    if sync == SynchronizationCode::SYN_REPORT =>
                {
                    let bytes = report.to_bytes();
                    hid.write_all(&bytes)
                        .context("Failed to write HID report")?;
                    report.reset_motion();
                }

                _ => {}
            }
        }
    }
}
