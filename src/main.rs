use chrono::Local;
use env_logger::Builder;
use evdev::{Device, EventSummary, KeyCode, RelativeAxisCode};
use log::{debug, error, info};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use std::fs::OpenOptions;
use std::io::{Error, Write};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

const INPUT_PATH: &str = "/dev/input/event0";
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
    fn packet(&self) -> [u8; 5] {
        [
            self.btn,
            self.x as u8,
            self.y as u8,
            self.wheel as u8,
            self.hwheel as u8,
        ]
    }

    fn clear(&mut self) {
        self.x = 0;
        self.y = 0;
        self.wheel = 0;
        self.hwheel = 0;
    }
}

fn init_log() {
    Builder::from_env(env_logger::Env::default())
        .format(|buf, rec| {
            writeln!(
                buf,
                "{} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                rec.level(),
                rec.args()
            )
        })
        .init();
}

fn write_gadget(
    rep: Arc<Mutex<Report>>,
    gadget: Arc<Mutex<std::fs::File>>,
    receiver: mpsc::Receiver<()>,
) {
    info!("Writer thread started");
    while receiver.recv().is_ok() {
        let packet = {
            let mut state = rep.lock().unwrap();
            let pkt = state.packet();
            state.clear();
            pkt
        };

        if let Ok(mut dev) = gadget.lock() {
            if let Err(e) = dev.write_all(&packet) {
                error!("Error writing to gadget: {}", e);
            } else {
                debug!("Wrote packet: {:?}", packet);
            }
        }
    }
}

fn handle_events<I>(events: I, rep: &Arc<Mutex<Report>>, sender: &mpsc::Sender<()>)
where
    I: Iterator<Item = evdev::InputEvent>,
{
    for event in events {
        let mut state = rep.lock().unwrap();
        match event.destructure() {
            EventSummary::RelativeAxis(_rel_event, axis, value) => match axis {
                RelativeAxisCode::REL_X => {
                    debug!("X: {}", value);
                    state.x = value as i8;
                }
                RelativeAxisCode::REL_Y => {
                    debug!("Y: {}", value);
                    state.y = value as i8;
                }
                RelativeAxisCode::REL_WHEEL => {
                    debug!("Wheel: {}", value);
                    state.wheel = value as i8;
                }
                RelativeAxisCode::REL_HWHEEL => {
                    debug!("HWheel: {}", value);
                    state.hwheel = value as i8;
                }
                _ => {}
            },
            EventSummary::Key(_key_event, key, value) => {
                debug!("Key {:?} value {}", key, value);
                update_key(key, value, &mut state.btn);
            }
            _ => {}
        }
        sender.send(()).unwrap();
    }
}

fn update_key(key: KeyCode, value: i32, btn: &mut u8) {
    let pressed = value == 1;
    match key {
        KeyCode::BTN_LEFT => modify_btn(pressed, btn, 0x01),
        KeyCode::BTN_RIGHT => modify_btn(pressed, btn, 0x02),
        KeyCode::BTN_MIDDLE => modify_btn(pressed, btn, 0x04),
        KeyCode::BTN_SIDE | KeyCode::BTN_BACK => modify_btn(pressed, btn, 0x08),
        KeyCode::BTN_EXTRA | KeyCode::BTN_FORWARD => modify_btn(pressed, btn, 0x10),
        _ => {}
    }
}

fn modify_btn(pressed: bool, btn: &mut u8, mask: u8) {
    if pressed {
        *btn |= mask;
    } else {
        *btn &= !mask;
    }
}

fn main() -> Result<(), Error> {
    init_log();
    info!("Starting mouse adapter");

    let mut device = Device::open(INPUT_PATH).map_err(|e| {
        error!("Failed to open {}: {}", INPUT_PATH, e);
        e
    })?;

    device.grab().map_err(|e| {
        error!("Failed to grab device: {}", e);
        e
    })?;
    info!("Device grabbed");

    let gadget = Arc::new(Mutex::new(
        OpenOptions::new()
            .write(true)
            .open(WRITE_PATH)
            .map_err(|e| {
                error!("Failed to open {}: {}", WRITE_PATH, e);
                e
            })?,
    ));
    info!("Gadget opened");

    let rep = Arc::new(Mutex::new(Report::default()));
    let (sender, receiver) = mpsc::channel();

    {
        let rep_clone = Arc::clone(&rep);
        let gad_clone = Arc::clone(&gadget);
        thread::spawn(move || write_gadget(rep_clone, gad_clone, receiver));
    }
    info!("Writer thread started");

    let fd = unsafe { BorrowedFd::borrow_raw(device.as_raw_fd()) };
    let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];
    info!("Entering event loop");

    loop {
        if let Err(e) = poll(&mut fds, PollTimeout::NONE) {
            error!("Poll error: {}", e);
            continue;
        }
        if let Ok(events) = device.fetch_events() {
            handle_events(events, &rep, &sender);
        }
    }
}
