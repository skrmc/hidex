use std::fs::OpenOptions;
use std::io::{Write, Error};
use evdev::{Device, InputEventKind, Key, RelativeAxisType};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use log::{info, error, debug};
use env_logger::Builder;
use chrono::Local;

const INPUT_PATH: &str = "/dev/input/event0";
const WRITE_PATH: &str = "/dev/hidg1";

#[derive(Default)]
struct MouseReport {
    buttons: u8,
    x: i8,
    y: i8,
    wheel: i8,
    hwheel: i8,
}

impl MouseReport {
    fn create_packet(&self) -> [u8; 5] {
        [
            self.buttons,
            self.x as u8,
            self.y as u8,
            self.wheel as u8,
            self.hwheel as u8,
        ]
    }

    fn reset(&mut self) {
        self.x = 0;
        self.y = 0;
        self.wheel = 0;
        self.hwheel = 0;
    }
}

fn init_logger() {
    Builder::from_env(env_logger::Env::default())
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.args()
            )
        })
        .init();
}

fn main() -> Result<(), Error> {
    init_logger();
    info!("Starting mouse adapter");

    let mut dev = match Device::open(INPUT_PATH) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to open input device {}: {}", INPUT_PATH, e);
            return Err(e.into());
        }
    };

    if let Err(e) = dev.grab() {
        error!("Failed to grab device: {}", e);
        return Err(e.into());
    }
    info!("Successfully grabbed input device");

    let gadget = Arc::new(Mutex::new(match OpenOptions::new().write(true).open(WRITE_PATH) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open gadget device {}: {}", WRITE_PATH, e);
            return Err(e);
        }
    }));
    info!("Successfully opened gadget device");

    let report = Arc::new(Mutex::new(MouseReport::default()));
    let (tx, rx) = mpsc::channel();

    let report_clone = Arc::clone(&report);
    let gadget_clone = Arc::clone(&gadget);

    thread::spawn(move || write_gadget(report_clone, gadget_clone, rx));
    info!("Started writer thread");

    info!("Entering main event loop");
    loop {
        match dev.fetch_events() {
            Ok(events) => handle_events(events, &report, &tx),
            Err(e) => {
                debug!("No events available: {}", e);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn write_gadget(report: Arc<Mutex<MouseReport>>, gadget: Arc<Mutex<std::fs::File>>, rx: mpsc::Receiver<()>) {
    info!("Writer thread started");
    loop {
        rx.recv().unwrap();
        let packet = {
            let mut r = report.lock().unwrap();
            let pkt = r.create_packet();
            r.reset();
            pkt
        };

        if let Ok(mut g) = gadget.lock() {
            if g.write_all(&packet).is_err() {
                error!("Error writing to gadget");
            } else {
                debug!("Wrote packet: {:?}", packet);
            }
        }
    }
}

fn handle_events<I>(events: I, report: &Arc<Mutex<MouseReport>>, tx: &mpsc::Sender<()>)
where
    I: Iterator<Item = evdev::InputEvent>,
{
    for ev in events {
        let mut r = report.lock().unwrap();
        match ev.kind() {
            InputEventKind::RelAxis(axis) => match axis {
                RelativeAxisType::REL_X => {
                    debug!("X movement: {}", ev.value());
                    r.x = ev.value() as i8;
                },
                RelativeAxisType::REL_Y => {
                    debug!("Y movement: {}", ev.value());
                    r.y = ev.value() as i8;
                },
                RelativeAxisType::REL_WHEEL => {
                    debug!("Wheel movement: {}", ev.value());
                    r.wheel = ev.value() as i8
                },
                RelativeAxisType::REL_HWHEEL => {
                    debug!("Horizontal wheel movement: {}", ev.value());
                    r.hwheel = ev.value() as i8
                },
                _ => {}
            },
            InputEventKind::Key(k) => {
                debug!("Button event: {:?}, value: {}", k, ev.value());
                handle_button(k, ev.value(), &mut r.buttons);
            }
            _ => {}
        }
        tx.send(()).unwrap(); // Notify the writer thread
    }
}

fn handle_button(key: Key, value: i32, buttons: &mut u8) {
    let pressed = value == 1;
    match key {
        Key::BTN_LEFT => update_button(pressed, buttons, 0x01),
        Key::BTN_RIGHT => update_button(pressed, buttons, 0x02),
        Key::BTN_MIDDLE => update_button(pressed, buttons, 0x04),
        Key::BTN_SIDE | Key::BTN_BACK => update_button(pressed, buttons, 0x08),
        Key::BTN_EXTRA | Key::BTN_FORWARD => update_button(pressed, buttons, 0x10),
        _ => {}
    }
}

fn update_button(pressed: bool, buttons: &mut u8, mask: u8) {
    if pressed {
        *buttons |= mask;
    } else {
        *buttons &= !mask;
    }
}