use std::time::Duration;
use crossterm::event::{self, Event, KeyCode};
use crate::app::App;
use crate::ui;

pub fn run(app: App) -> anyhow::Result<()> {
    // 1. Setup Terminal
    let mut terminal = ratatui::init();

    // 2. The Game Loop
    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        // Handle Input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    // We will add more keys here later (j, k, Enter)
                    _ => {} 
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // 3. Restore Terminal
    ratatui::restore();
    Ok(())
}