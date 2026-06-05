//! Pure layout: a verified document's scene IR to width-wrapped text lines.
//!
//! This is the testable core of the viewer. It takes an engine `Scene` and a
//! target width and produces a flat
//! `Vec<String>` of display lines, word-wrapped to the width. It performs no
//! terminal I/O and holds no state, so it is covered by golden tests exactly
//! like the engine's renderers.
//!
//! The block conventions mirror the engine's plain-text renderer
//! (`entangled_engine::text`): heading prefix `#`, mark delimiters
//! `*`/`/`/`` ` ``/`~`, list bullets, `> ` quotes, `[image: alt]`, link
//! `text (-> target)`, and so on. As there, attacker-controlled strings are
//! escaped so a literal marker cannot forge structure.
//!
//! Unlike the plain-text renderer, lines are wrapped to the viewport width.
//! When a block carries a leading marker, its wrapped continuation rows keep it
//! aligned: a wrapped quote repeats `> `, a wrapped list item indents under its
//! bullet, and a wrapped form field keeps its two-space indent.
//!
//! This is a content viewer, not a conforming Entangled client: it renders the
//! content area only. The trust-state, canary, and PIP chrome required by
//! section 10 are out of scope here (the scene IR does not carry them); the
//! viewer surfaces a static, honest "not a conforming client" chrome label.

use entangled_engine::{FormFieldView, InlineRun, LinkRef, Scene, SceneNode, TextStyle};

/// A pre-wrap line: prose that is word-wrapped to the viewport, or verbatim
/// content (code, fences) that is emitted intact regardless of width.
///
/// A wrapped line carries a `continuation` prefix repeated at the start of each
/// row after the first, so a wrapped quote keeps its `> ` and a wrapped list
/// item or form field stays aligned under its bullet/indent.
enum LogicalLine {
    Wrap { text: String, continuation: String },
    Verbatim(String),
}

/// A wrapped line with no continuation prefix (the common case).
fn wrap(text: String) -> LogicalLine {
    LogicalLine::Wrap {
        text,
        continuation: String::new(),
    }
}

/// A wrapped line whose wrapped rows are prefixed with `continuation`.
fn wrap_cont(text: String, continuation: impl Into<String>) -> LogicalLine {
    LogicalLine::Wrap {
        text,
        continuation: continuation.into(),
    }
}

/// Lay a scene out into display lines wrapped to `width` columns.
///
/// `width` is the number of columns available for content. A width of zero is
/// treated as one to avoid an empty wrap. Prose lines are word-wrapped; code
/// block content and its fences are emitted verbatim and never wrapped.
pub fn lay_out(scene: &Scene, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut logical = Vec::new();
    for node in &scene.nodes {
        node_lines(node, &mut logical);
    }
    let mut out = Vec::new();
    for line in logical {
        match line {
            LogicalLine::Wrap { text, continuation } => {
                wrap_into(&text, &continuation, width, &mut out)
            }
            LogicalLine::Verbatim(s) => out.push(s),
        }
    }
    out
}

/// Emit the logical (pre-wrap) lines for one node, including a trailing blank
/// line as a block separator.
fn node_lines(node: &SceneNode, out: &mut Vec<LogicalLine>) {
    // A blank separator line; blank prose so it survives wrapping unchanged.
    let blank = || wrap(String::new());
    match node {
        SceneNode::Paragraph { runs } => {
            out.push(wrap(runs_to_string(runs)));
            out.push(blank());
        }
        SceneNode::Heading { level, runs } => {
            let hashes = "#".repeat(*level as usize);
            out.push(wrap(format!("{hashes} {}", runs_to_string(runs))));
            out.push(blank());
        }
        SceneNode::CodeBlock { language, text } => {
            let fence = fence_for(text);
            // Fence and code content are verbatim: never word-wrapped.
            out.push(LogicalLine::Verbatim(format!(
                "{fence}{}",
                language.as_str()
            )));
            for line in text.split('\n') {
                out.push(LogicalLine::Verbatim(line.to_owned()));
            }
            // split('\n') on a trailing-newline string yields a final empty
            // element; drop it so the fence is flush against the last code line.
            if text.ends_with('\n') {
                out.pop();
            }
            out.push(LogicalLine::Verbatim(fence));
            out.push(blank());
        }
        SceneNode::Quote { runs, attribution } => {
            // Wrapped quote rows keep the `> ` marker.
            out.push(wrap_cont(format!("> {}", runs_to_string(runs)), "> "));
            if let Some(attr) = attribution {
                out.push(wrap_cont(format!("> -- {}", runs_to_string(attr)), "> "));
            }
            out.push(blank());
        }
        SceneNode::List { ordered, items } => {
            for (i, item) in items.iter().enumerate() {
                let bullet = if *ordered {
                    format!("{}. ", i + 1)
                } else {
                    "- ".to_owned()
                };
                // Wrapped item rows align under the text, past the bullet.
                let indent = " ".repeat(bullet.chars().count());
                out.push(wrap_cont(
                    format!("{bullet}{}", runs_to_string(item)),
                    indent,
                ));
            }
            out.push(blank());
        }
        SceneNode::Divider => {
            out.push(wrap("---".to_owned()));
            out.push(blank());
        }
        SceneNode::Image { image } => {
            if image.alt.is_empty() {
                out.push(wrap("[image]".to_owned()));
            } else {
                out.push(wrap(format!("[image: {}]", escape(&image.alt))));
            }
            if let Some(caption) = &image.caption {
                out.push(wrap(format!("({})", escape(caption))));
            }
            out.push(blank());
        }
        SceneNode::Link { label, link } => {
            out.push(wrap(format!(
                "{} (-> {})",
                runs_to_string(label),
                link_target(link)
            )));
            out.push(blank());
        }
        SceneNode::SubmitForm {
            label,
            submit_to,
            fields,
            submit_label,
        } => {
            out.push(wrap(format!(
                "{} (form -> {})",
                runs_to_string(label),
                submit_to.as_str()
            )));
            for field in fields {
                // Wrapped field rows keep the two-space field indent.
                out.push(wrap_cont(format!("  {}", form_field(field)), "  "));
            }
            out.push(wrap(format!("[{}]", escape(submit_label))));
            out.push(blank());
        }
        SceneNode::Feedback { variant, runs } => {
            out.push(wrap(format!(
                "[{}] {}",
                feedback_variant(variant),
                runs_to_string(runs)
            )));
            out.push(blank());
        }
        SceneNode::Note {
            variant,
            title,
            runs,
        } => {
            let head = match title {
                Some(t) => format!("[{}] {}", note_variant(variant), escape(t)),
                None => format!("[{}]", note_variant(variant)),
            };
            out.push(wrap(head));
            out.push(wrap(runs_to_string(runs)));
            out.push(blank());
        }
    }
}

fn runs_to_string(runs: &[InlineRun]) -> String {
    let mut s = String::new();
    for run in runs {
        match run {
            InlineRun::Text { text, style } => s.push_str(&styled(text, *style)),
            InlineRun::Link { text, style, link } => {
                s.push_str(&styled(text, *style));
                s.push_str(&format!(" (-> {})", link_target(link)));
            }
        }
    }
    s
}

fn styled(text: &str, style: TextStyle) -> String {
    let mut s = escape(text);
    if style.strikethrough {
        s = format!("~{s}~");
    }
    if style.code {
        s = format!("`{s}`");
    }
    if style.italic {
        s = format!("/{s}/");
    }
    if style.bold {
        s = format!("*{s}*");
    }
    s
}

fn link_target(link: &LinkRef) -> String {
    match link {
        LinkRef::SameSite { path } => path.as_str().to_owned(),
        LinkRef::Entangled { address, path, .. } => {
            format!("{}{}", address.as_str(), path.as_str())
        }
        LinkRef::Carrier { url, .. } => escape_url(url),
        LinkRef::Citation { url } => escape_url(url),
    }
}

fn form_field(field: &FormFieldView) -> String {
    let (kind, name, label, required) = match field {
        FormFieldView::Text {
            name,
            label,
            required,
            ..
        } => ("text", name, label, *required),
        FormFieldView::Textarea {
            name,
            label,
            required,
            ..
        } => ("textarea", name, label, *required),
        FormFieldView::Select {
            name,
            label,
            required,
            ..
        } => ("select", name, label, *required),
        FormFieldView::Checkbox {
            name,
            label,
            required,
        } => ("checkbox", name, label, *required),
    };
    let req = if required { " (required)" } else { "" };
    format!(
        "[{}] {} = \"{}\"{}",
        kind,
        name.as_str(),
        escape(label),
        req
    )
}

fn feedback_variant(v: &entangled_core::types::FeedbackVariant) -> &'static str {
    use entangled_core::types::FeedbackVariant as V;
    match v {
        V::Success => "success",
        V::Info => "info",
        V::Warning => "warning",
        V::Error => "error",
    }
}

fn note_variant(v: &entangled_core::types::NoteVariant) -> &'static str {
    use entangled_core::types::NoteVariant as V;
    match v {
        V::Info => "info",
        V::Warning => "warning",
        V::Danger => "danger",
        V::Success => "success",
    }
}

/// Backslash-escape the renderer's markers in free-form text.
fn escape(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '*' | '/' | '`' | '~' | '[' | ']' | '(' | ')' | '"'
        ) {
            s.push('\\');
        }
        s.push(ch);
    }
    s
}

/// Escape only the markers that could close a link's `(-> ...)` wrapper.
fn escape_url(url: &str) -> String {
    let mut s = String::with_capacity(url.len());
    for ch in url.chars() {
        if matches!(ch, '\\' | ')' | ']') {
            s.push('\\');
        }
        s.push(ch);
    }
    s
}

/// A backtick fence one longer than the longest backtick run in `text`.
fn fence_for(text: &str) -> String {
    let mut longest = 0usize;
    let mut current = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    "`".repeat(longest.max(2) + 1)
}

/// Word-wrap `line` to `width` columns, pushing each wrapped row into `out`.
///
/// The first row is emitted as-is (the caller has already put any leading
/// marker, such as `> ` or `1. `, in `line`). Every row after the first is
/// prefixed with `continuation` so wrapped quotes keep their marker and wrapped
/// list items / form fields stay aligned; the wrap width for those rows is
/// reduced by the continuation's width. A blank input line produces one blank
/// output row. Wrapping is by Unicode scalar count (a deliberate approximation;
/// grapheme/display-width handling is a later refinement).
fn wrap_into(line: &str, continuation: &str, width: usize, out: &mut Vec<String>) {
    if line.is_empty() {
        out.push(String::new());
        return;
    }
    let cont_len = continuation.chars().count();
    let first = out.len();
    let mut current = String::new();
    let mut current_len = 0usize;
    // Width available on the current row: full width on the first row, reduced
    // by the continuation prefix on later rows. Clamped to at least 1.
    let row_width = |row_index: usize| {
        if row_index == 0 {
            width
        } else {
            width.saturating_sub(cont_len).max(1)
        }
    };
    for word in line.split(' ') {
        let word_len = word.chars().count();
        let w = row_width(out.len() - first);
        if current_len == 0 {
            // First word on the row: place it even if it exceeds the width
            // (a single over-long word is hard-broken below).
            push_word(word, w, &mut current, &mut current_len, out);
        } else if current_len + 1 + word_len <= w {
            current.push(' ');
            current.push_str(word);
            current_len += 1 + word_len;
        } else {
            out.push(std::mem::take(&mut current));
            current_len = 0;
            let w = row_width(out.len() - first);
            push_word(word, w, &mut current, &mut current_len, out);
        }
    }
    if !current.is_empty() || current_len == 0 {
        out.push(current);
    }
    // Prefix every row after the first with the continuation.
    if !continuation.is_empty() {
        for row in out.iter_mut().skip(first + 1) {
            row.insert_str(0, continuation);
        }
    }
}

/// Place `word` at the start of a fresh row, hard-breaking it across rows if it
/// is wider than `width`.
fn push_word(
    word: &str,
    width: usize,
    current: &mut String,
    current_len: &mut usize,
    out: &mut Vec<String>,
) {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() <= width {
        current.push_str(word);
        *current_len = chars.len();
        return;
    }
    // Hard-break the over-long word into width-sized chunks.
    for chunk in chars.chunks(width) {
        let piece: String = chunk.iter().collect();
        if chunk.len() == width {
            out.push(piece);
        } else {
            *current = piece;
            *current_len = chunk.len();
        }
    }
}
