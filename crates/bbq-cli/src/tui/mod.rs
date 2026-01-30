mod app;
mod constants;
mod render;
mod types;
mod worker;

use std::io::{self, Stdout};

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::prelude::*;

use app::App;
use render::ui;

pub(crate) fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let app = App::new();
    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("error: {err}");
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, mut app: App) -> io::Result<()> {
    loop {
        app.update_status();
        app.handle_worker_events();
        terminal.draw(|frame| ui(frame, &mut app))?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if app.is_update_prompt_mode() {
                    if app.handle_update_prompt_key(key) {
                        app.persist_restore_state();
                        return Ok(());
                    }
                } else if app.is_setup_mode() {
                    if app.handle_setup_key(key) {
                        app.persist_restore_state();
                        return Ok(());
                    }
                } else if app.is_input_mode() {
                    app.handle_input(key);
                } else if app.handle_key(key) {
                    app.persist_restore_state();
                    return Ok(());
                }
            }
        }
    }
}
