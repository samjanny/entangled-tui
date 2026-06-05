//! `entangled-tui` binary: the thin terminal shell.
//!
//! Loads a content document, lowers it to a scene with `entangled-engine`, and
//! displays it in a scrollable terminal view. All rendering logic lives in the
//! library (`layout`, `app`); this file is only the crossterm/ratatui
//! event-loop and draw glue.
//!
//! # Verification boundary
//!
//! This viewer assumes its input is already verified, exactly as the engine
//! does. It deserializes a `ContentDocument` from JSON (which re-applies the
//! type invariants of the core newtypes) but does NOT verify the Ed25519
//! signature: signature/trust verification is the job of a real client built
//! on `entangled-core`, not of a content viewer. The viewer is for inspecting
//! the rendered content of a document one already trusts.

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::process::ExitCode;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use entangled_core::types::ContentDocument;
use entangled_engine::Scene;
use entangled_tui::{App, CHROME_LABEL};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: entangled-tui <content-document.json>");
        eprintln!("  the JSON must be a verified Entangled content document.");
        return ExitCode::from(2);
    };
    let path = PathBuf::from(path);

    let scene = match load_scene(&path) {
        Ok(scene) => scene,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::FAILURE;
        }
    };

    match run(scene) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Read a content document from `path` and lower it to a scene. Deserialization
/// re-applies the core type invariants; the signature is not checked here (see
/// the module-level verification-boundary note).
fn load_scene(path: &std::path::Path) -> Result<Scene, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let doc: ContentDocument =
        serde_json::from_slice(&bytes).map_err(|e| format!("parsing content document: {e}"))?;
    Ok(Scene::from_content(&doc))
}

/// Set up the terminal, run the event loop, and restore the terminal on exit
/// (including on error, so a panic-free error does not leave the terminal in
/// raw mode).
fn run(scene: Scene) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &scene);

    // Always restore, regardless of the loop's result.
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
