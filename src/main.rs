mod app;
mod k8s;
mod tui;
mod ui;

use app::App;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize State (Network calls happen inside here)
    let app = App::new().await?;

    // 2. Hand over control to the TUI Loop
    tui::run(app)?;

    Ok(())
}