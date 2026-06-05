//! The interactive terminal viewer: the crossterm/ratatui event-loop and draw
//! shell over the pure [`App`] state.
//!
//! This is the one part of the crate that touches the terminal. It is kept here
//! (rather than in the binary's `main`) so both the `entangled-tui` binary and
//! the crate's examples can drive a [`Scene`] through the same viewer.

use std::io::{self, Stdout};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use entangled_engine::Scene;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal};

use crate::{App, CHROME_LABEL};

/// Run the interactive viewer on `scene` until the user quits.
///
/// Sets up the alternate screen and raw mode, runs the event loop, and restores
/// the terminal on exit - including on error, so a failure does not leave the
/// terminal in raw mode.
pub fn run(scene: &Scene) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, scene);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, scene: &Scene) -> io::Result<()> {
    // Initial width from the current terminal size; re-wrapped on resize.
    let initial_width = content_width(terminal.size()?.width);
    let mut app = App::new(scene, initial_width);

    loop {
        terminal.draw(|f| draw(f, &app))?;

        // The content viewport height: total rows minus chrome (1) and the
        // bordered content block's two borders (2) and status (1).
        let height = content_height(terminal.size()?.height);

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Down | KeyCode::Char('j') => app.scroll_down(1, height),
                KeyCode::Up | KeyCode::Char('k') => app.scroll_up(1),
                KeyCode::PageDown | KeyCode::Char(' ') => app.scroll_down(height, height),
                KeyCode::PageUp => app.scroll_up(height),
                KeyCode::Home | KeyCode::Char('g') => app.to_top(),
                KeyCode::End | KeyCode::Char('G') => app.to_bottom(height),
                _ => {}
            },
            Event::Resize(cols, _rows) => {
                app.set_width(scene, content_width(cols));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Content columns available inside the bordered block (two border columns).
fn content_width(term_cols: u16) -> usize {
    (term_cols as usize).saturating_sub(2).max(1)
}

/// Content rows available inside the bordered block, below the chrome line and
/// above the status line.
fn content_height(term_rows: u16) -> usize {
    // chrome (1) + content borders (2) + status (1) = 4 rows of overhead.
    (term_rows as usize).saturating_sub(4).max(1)
}

fn draw(f: &mut Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // chrome label
            Constraint::Min(1),    // content
            Constraint::Length(1), // status
        ])
        .split(f.area());

    // Chrome: the honest static label, visually distinct from content.
    let chrome = Paragraph::new(Line::from(Span::styled(
        CHROME_LABEL,
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    f.render_widget(chrome, chunks[0]);

    // Content: the visible slice of laid-out lines inside a border.
    let inner_height = chunks[1].height.saturating_sub(2) as usize;
    let visible: Vec<Line<'_>> = app
        .visible(inner_height)
        .iter()
        .map(|l| Line::from(l.clone()))
        .collect();
    let content =
        Paragraph::new(visible).block(Block::default().borders(Borders::ALL).title("content"));
    f.render_widget(content, chunks[1]);

    // Status: scroll position and key hints.
    let status = Paragraph::new(Line::from(format!(
        " {}/{} lines  -  j/k scroll, g/G top/bottom, q quit",
        app.scroll(),
        app.line_count()
    )))
    .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}
