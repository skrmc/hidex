use evdev::{Device, EventSummary, KeyCode, RelativeAxisCode, SynchronizationCode};
use log::{debug, error, info};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::fs::OpenOptions;
use std::io::{Error, Write};
use std::os::fd::{AsRawFd, BorrowedFd};

const INPUT_PATH: &str = "/dev/input/event1";
const WRITE_PATH: &str = "/dev/hidg1";

#[derive(Default)]
struct Report {
    btn: u8,
    x: i8,
    y: i8,
    wheel: i8,
    hwheel: i8,
}

impl Report {
    #[inline]
    fn packet(&self) -> [u8; 5] {
        [
            self.btn,
            self.x as u8,
            self.y as u8,
            self.wheel as u8,
            self.hwheel as u8,
        ]
    }
    #[inline]
    fn reset_motion(&mut self) {
        self.x = 0;
        self.y = 0;
        self.wheel = 0;
        self.hwheel = 0;
    }
}

#[inline]
fn clamp_i8(v: i32) -> i8 {
    v.clamp(i8::MIN as i32, i8::MAX as i32) as i8
}

fn main() -> Result<(), Error> {
    env_logger::init();
    info!("Starting single-thread mouse adapter");

    let mut dev = Device::open(INPUT_PATH).map_err(|e| {
        error!("Open {INPUT_PATH} failed: {e}");
        e
    })?;

    dev.grab().map_err(|e| {
        error!("Grab failed: {e}");
        e
    })?;
    info!("Input device grabbed");

    let mut hid = OpenOptions::new()
        .write(true)
        .open(WRITE_PATH)
        .map_err(|e| {
            error!("Open {WRITE_PATH} failed: {e}");
            e
        })?;
    info!("HID gadget opened");

    let mut report = Report::default();

    let fd = unsafe { BorrowedFd::borrow_raw(dev.as_raw_fd()) };
    let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
    info!("Entering event loop");

    loop {
        if let Err(e) = poll(&mut fds, PollTimeout::NONE) {
            error!("poll error: {e}");
            continue;
        }

        if let Ok(events) = dev.fetch_events() {
            for ev in events {
                match ev.destructure() {
                    EventSummary::RelativeAxis(_, code, value) => match code {
                        RelativeAxisCode::REL_X => report.x = clamp_i8(value),
                        RelativeAxisCode::REL_Y => report.y = clamp_i8(value),
                        RelativeAxisCode::REL_WHEEL => report.wheel = clamp_i8(value),
                        RelativeAxisCode::REL_HWHEEL => report.hwheel = clamp_i8(value),
                        _ => {}
                    },

                    EventSummary::Key(_, key, val) => {
                        let pressed = val == 1;
                        match key {
                            KeyCode::BTN_LEFT => modify_btn(&mut report.btn, pressed, 0x01),
                            KeyCode::BTN_RIGHT => modify_btn(&mut report.btn, pressed, 0x02),
                            KeyCode::BTN_MIDDLE => modify_btn(&mut report.btn, pressed, 0x04),
                            KeyCode::BTN_SIDE | KeyCode::BTN_BACK => {
                                modify_btn(&mut report.btn, pressed, 0x08)
                            }
                            KeyCode::BTN_EXTRA | KeyCode::BTN_FORWARD => {
                                modify_btn(&mut report.btn, pressed, 0x10)
                            }
                            _ => {}
                        }
                    }

                    EventSummary::Synchronization(_, sync, _) => {
                        if sync == SynchronizationCode::SYN_REPORT {
                            if let Err(e) = hid.write_all(&report.packet()) {
                                error!("write() failed: {e}");
                            } else {
                                debug!("pkt {:?}", report.packet());
                            }
                            report.reset_motion();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[inline]
fn modify_btn(byte: &mut u8, pressed: bool, mask: u8) {
    if pressed {
        *byte |= mask;
    } else {
        *byte &= !mask;
    }
}
