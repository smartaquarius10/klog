use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};
use crate::app::App;

pub fn render(f: &mut Frame, app: &App) {
    // 1. Layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Body (Table)
            Constraint::Length(3), // Footer / Prompt
        ])
        .split(f.area());

    // 2. Header
    let header_text = format!(" Cluster: [Local] | Namespace: {} | Total: {}", app.namespace, app.pods.len());
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title(" klog "));
    f.render_widget(header, chunks[0]);

    // 3. Body - The Pod Table
    // Define the Column Headers
    let header_cells = ["Name", "Namespace", "Status"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let table_header = Row::new(header_cells).height(1).bottom_margin(1);

    // Map logic: Convert Pod struct -> Table Row
    let rows = app.pods.iter().map(|pod| {
        let name = pod.metadata.name.clone().unwrap_or_default();
        let ns = pod.metadata.namespace.clone().unwrap_or_default();
        
        // Safe unwrap for nested Status fields
        let status = pod.status.as_ref()
            .and_then(|s| s.phase.clone())
            .unwrap_or("Unknown".to_string());

        // Colorize status (Simple version)
        let style = if status == "Running" {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };

        Row::new(vec![
            Cell::from(name),
            Cell::from(ns),
            Cell::from(status).style(style),
        ])
    });

    // Create the Table Widget
    let table = Table::new(rows, [
        Constraint::Percentage(40), // Name gets most space
        Constraint::Percentage(40), // Namespace
        Constraint::Percentage(20), // Status
    ])
    .header(table_header)
    .block(Block::default().borders(Borders::ALL).title(" Pods "));

    f.render_widget(table, chunks[1]);
}