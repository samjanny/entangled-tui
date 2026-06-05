//! Viewer state: the laid-out lines plus a scroll viewport.
//!
//! Pure and testable: scrolling is arithmetic over the line count and the
//! viewport height, with no terminal I/O. The thin shell (`main`) feeds it the
//! current height and key events and asks for the visible slice.

use entangled_engine::Scene;

use crate::layout::lay_out;

/// The viewer's content state: the wrapped lines and the current scroll offset.
pub struct App {
    lines: Vec<String>,
    /// Index of the first visible line.
    scroll: usize,
    /// The width the lines were laid out for, so a resize can re-wrap.
    width: usize,
}

impl App {
    /// Lay out `scene` at `width` columns and start scrolled to the top.
    pub fn new(scene: &Scene, width: usize) -> App {
        App {
            lines: lay_out(scene, width),
            scroll: 0,
            width,
        }
    }

    /// Total laid-out line count.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// The current first-visible-line index.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Re-wrap to a new width (on terminal resize), keeping the top line stable
    /// is not attempted; the offset is clamped to the new line count.
    pub fn set_width(&mut self, scene: &Scene, width: usize) {
        if width != self.width {
            self.lines = lay_out(scene, width);
            self.width = width;
            self.clamp(usize::MAX);
        }
    }

    /// The lines visible in a viewport of `height` rows, starting at the
    /// current scroll offset.
    pub fn visible(&self, height: usize) -> &[String] {
        let start = self.scroll.min(self.lines.len());
        let end = start.saturating_add(height).min(self.lines.len());
        &self.lines[start..end]
    }

    /// Scroll down by `n` lines, without scrolling past the last full screen.
    pub fn scroll_down(&mut self, n: usize, height: usize) {
        self.scroll = self.scroll.saturating_add(n);
        self.clamp(height);
    }

    /// Scroll up by `n` lines.
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    /// Jump to the top.
    pub fn to_top(&mut self) {
        self.scroll = 0;
    }

    /// Jump to the last full screen of `height` rows.
    pub fn to_bottom(&mut self, height: usize) {
        self.scroll = self.max_scroll(height);
    }

    /// The largest scroll offset that still shows content, for a viewport of
    /// `height` rows: the last line is reachable but the view does not scroll
    /// into empty space beyond it.
    fn max_scroll(&self, height: usize) -> usize {
        self.lines.len().saturating_sub(height)
    }

    /// Clamp the scroll offset to `max_scroll(height)`. A height of
    /// `usize::MAX` clamps only to the line count (used on re-wrap when the
    /// height is not known here).
    fn clamp(&mut self, height: usize) {
        let max = if height == usize::MAX {
            self.lines.len().saturating_sub(1)
        } else {
            self.max_scroll(height)
        };
        self.scroll = self.scroll.min(max);
    }
}
