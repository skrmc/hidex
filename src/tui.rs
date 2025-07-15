use std::{fs, io, path::PathBuf};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use evdev::Device;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

const INPUT_DIR: &str = "/dev/input";

type Backend = CrosstermBackend<io::Stdout>;
type Term = Terminal<Backend>;

// Single entry in the device list shown in the TUI.
#[derive(Clone)]
struct DeviceEntry {
    path: PathBuf,
    name: String,
}

// Application state for the device picker.
struct App {
    devices: Vec<DeviceEntry>,
    selected: usize,
}

impl App {
    fn new() -> io::Result<Self> {
        Ok(Self {
            devices: scan_devices()?,
            selected: 0,
        })
    }

    fn refresh(&mut self) -> io::Result<()> {
        self.devices = scan_devices()?;
        if self.selected >= self.devices.len() {
            self.selected = self.devices.len().saturating_sub(1);
        }
        Ok(())
    }

    fn selected_device(&self) -> Option<&DeviceEntry> {
        self.devices.get(self.selected)
    }
}

/* Public entry point: run the TUI picker and return the chosen device.
 * Returns:
 * - Ok(Some(path)) if the user selected a device
 * - Ok(None) if the user pressed 'q' to quit
 */
pub fn pick_device() -> io::Result<Option<PathBuf>> {
    // Enter raw mode and the alternate screen
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = Backend::new(stdout);
    let mut terminal = Term::new(backend)?;

    // Use an inner closure so we can always restore the terminal afterwards.
    let result = (|| {
        let mut app = App::new()?;
        run(&mut terminal, &mut app)
    })();

    // Always try to restore terminal state, even if the app failed.
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

// Main TUI loop.
fn run(terminal: &mut Term, app: &mut App) -> io::Result<Option<PathBuf>> {
    loop {
        // Draw the UI
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(frame.area());

            // Header
            let header = Paragraph::new("Select an input device").block(
                Block::default()
                    .title("Device Picker")
                    .borders(Borders::ALL),
            );
            frame.render_widget(header, chunks[0]);

            // Device list
            let items: Vec<ListItem> = if app.devices.is_empty() {
                vec![ListItem::new("No /dev/input/event* devices found")]
            } else {
                app.devices
                    .iter()
                    .map(|device| {
                        let text = format!("{} ({})", device.path.display(), device.name);
                        ListItem::new(text)
                    })
                    .collect()
            };

            let mut state = ListState::default();
            if !app.devices.is_empty() {
                state.select(Some(app.selected));
            }

            let list = List::new(items)
                .block(Block::default().title("/dev/input").borders(Borders::ALL))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

            frame.render_stateful_widget(list, chunks[1], &mut state);

            // Footer
            let footer_text = "↑/↓: move  Enter: select  r: refresh  q: quit";
            let footer = Paragraph::new(footer_text);
            frame.render_widget(footer, chunks[2]);
        })?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') => return Ok(None),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                KeyCode::Char('r') => app.refresh()?,

                KeyCode::Up => {
                    if !app.devices.is_empty() {
                        if app.selected == 0 {
                            app.selected = app.devices.len() - 1;
                        } else {
                            app.selected -= 1;
                        }
                    }
                }

                KeyCode::Down => {
                    if !app.devices.is_empty() {
                        app.selected = (app.selected + 1) % app.devices.len();
                    }
                }

                KeyCode::Enter => {
                    if let Some(device) = app.selected_device() {
                        return Ok(Some(device.path.clone()));
                    }
                }

                _ => {}
            }
        }
    }
}

// Scan /dev/input/event* and collect their names.
fn scan_devices() -> io::Result<Vec<DeviceEntry>> {
    let mut devices = Vec::new();

    for entry in fs::read_dir(INPUT_DIR)? {
        let entry = entry?;
        let path = entry.path();

        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !file_name.starts_with("event") {
            continue;
        }

        if let Ok(dev) = Device::open(&path) {
            let name = dev.name().unwrap_or("Unknown device").to_string();
            devices.push(DeviceEntry { path, name });
        }
    }

    devices.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(devices)
}
