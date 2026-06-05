//! Pure layout: a verified document's scene IR to width-wrapped styled lines.
//!
//! This is the testable core of the viewer. It takes an engine `Scene` and a
//! target width and produces a flat `Vec<StyledLine>` of display rows, each a
//! sequence of [`StyledSpan`]s carrying their style as data. It performs no
//! terminal I/O and holds no state, so it is covered by golden tests exactly
//! like the engine's renderers.
//!
//! Inline marks (bold/italic/code/strikethrough) are carried as attributes on
//! the spans, not flattened into delimiter characters - the viewer turns them
//! into real terminal styling. Block roles (heading, quote, link, code) are
//! carried as a [`Role`] so the viewer can color them; this module stays
//! color-free and toolkit-neutral.
//!
//! Some structural markers are kept as literal text because the terminal has no
//! styling that conveys them: the `> ` quote prefix, list bullets (`1. ` /
//! `- `), `[image: alt]`, and the `(-> target)` / `(form -> path)` link and
//! form annotations. These are emitted as [`Role::Marker`] spans.
//!
//! Lines are word-wrapped to the viewport width. When a block carries a leading
//! marker, its wrapped continuation rows keep it aligned: a wrapped quote
//! repeats `> `, a wrapped list item indents under its bullet, and a wrapped
//! form field keeps its two-space indent. Code block content and its adaptive
//! fence are emitted verbatim and never wrapped. Wrapping is by Unicode scalar
//! count (a deliberate approximation; grapheme/display-width handling is a
//! later refinement).
//!
//! This is a content viewer, not a conforming Entangled client: it renders the
//! content area only. The trust-state, canary, and PIP chrome required by
//! section 10 are out of scope here (the scene IR does not carry them); the
//! viewer surfaces a static, honest "not a conforming client" chrome label.

use entangled_engine::{FormFieldView, InlineRun, LinkRef, Scene, SceneNode, TextStyle};

/// Inline attributes carried from the document's text marks. These map to real
/// terminal attributes at the viewer; they are never rendered as delimiters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpanStyle {
    /// The `bold` mark.
    pub bold: bool,
    /// The `italic` mark.
    pub italic: bool,
    /// The `code` (monospace) mark.
    pub code: bool,
    /// The `strikethrough` mark.
    pub strikethrough: bool,
    /// The span's semantic role; the viewer chooses the color.
    pub role: Role,
}

impl SpanStyle {
    /// A plain span style with the given role and no inline attributes.
    fn role(role: Role) -> SpanStyle {
        SpanStyle {
            role,
            ..SpanStyle::default()
        }
    }

    /// Carry the document's inline marks (`TextStyle`) onto a span of `role`.
    fn from_marks(text_style: TextStyle, role: Role) -> SpanStyle {
        SpanStyle {
            bold: text_style.bold,
            italic: text_style.italic,
            code: text_style.code,
            strikethrough: text_style.strikethrough,
            role,
        }
    }
}

/// The semantic role of a span. The viewer maps each role to a color/emphasis;
/// this module assigns no colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    /// Ordinary body text.
    #[default]
    Plain,
    /// Heading text.
    Heading,
    /// Quote body and its `> ` marker.
    Quote,
    /// Link label and its `(-> target)` annotation.
    Link,
    /// Code fence and code content.
    CodeBlock,
    /// Structural punctuation kept as text (bullets, `[image: ...]`, `(form ->
    /// ...)`, the `[variant]` / field tags).
    Marker,
}

/// One styled run of text within a display row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyledSpan {
    /// The visible text of this span.
    pub text: String,
    /// How to style it.
    pub style: SpanStyle,
}

impl StyledSpan {
    fn new(text: impl Into<String>, style: SpanStyle) -> StyledSpan {
        StyledSpan {
            text: text.into(),
            style,
        }
    }
}

/// One display row: a sequence of styled spans. An empty row is a blank line.
pub type StyledLine = Vec<StyledSpan>;

/// A pre-wrap line: a fixed leading marker plus styled body spans that are
/// word-wrapped to the viewport, or verbatim rows (code, fences) emitted intact.
///
/// The `prefix` (e.g. `"> "`, `"## "`, `"1. "`) is a fixed leading marker that
/// is NOT word-wrapped: it is placed verbatim at the start of the first row,
/// while only the `body` spans are wrapped. On rows after the first, `prefix`
/// is replaced by `continuation` (e.g. `"> "` again for a quote, or aligned
/// indentation for a list item), so a wrapped block keeps its marker aligned.
enum LogicalLine {
    Wrap {
        prefix: Option<StyledSpan>,
        body: Vec<StyledSpan>,
        continuation: Option<StyledSpan>,
    },
    Verbatim(StyledLine),
}

/// Lay a scene out into styled display rows wrapped to `width` columns.
///
/// `width` is the number of columns available for content. A width of zero is
/// treated as one to avoid an empty wrap. Prose rows are word-wrapped; code
/// block content and its fences are emitted verbatim and never wrapped.
pub fn lay_out(scene: &Scene, width: usize) -> Vec<StyledLine> {
    let width = width.max(1);
    let mut logical = Vec::new();
    for node in &scene.nodes {
        node_lines(node, &mut logical);
    }
    let mut out = Vec::new();
    for line in logical {
        match line {
            LogicalLine::Wrap {
                prefix,
                body,
                continuation,
            } => wrap_into(
                prefix.as_ref(),
                &body,
                continuation.as_ref(),
                width,
                &mut out,
            ),
            LogicalLine::Verbatim(row) => out.push(row),
        }
    }
    out
}

/// A blank separator row.
fn blank() -> LogicalLine {
    LogicalLine::Wrap {
        prefix: None,
        body: Vec::new(),
        continuation: None,
    }
}

/// A wrapped line with no leading marker (the common case).
fn wrap(body: Vec<StyledSpan>) -> LogicalLine {
    LogicalLine::Wrap {
        prefix: None,
        body,
        continuation: None,
    }
}

/// A wrapped line with a fixed leading `prefix` marker and aligned
/// `continuation` on wrapped rows.
fn wrap_marked(prefix: StyledSpan, body: Vec<StyledSpan>, continuation: StyledSpan) -> LogicalLine {
    LogicalLine::Wrap {
        prefix: Some(prefix),
        body,
        continuation: Some(continuation),
    }
}

/// Emit the logical (pre-wrap) lines for one node, including a trailing blank
/// row as a block separator.
fn node_lines(node: &SceneNode, out: &mut Vec<LogicalLine>) {
    match node {
        SceneNode::Paragraph { runs } => {
            out.push(wrap(run_spans(runs, Role::Plain)));
            out.push(blank());
        }
        SceneNode::Heading { level, runs } => {
            // Heading text carries Role::Heading; the `#` prefix is a marker so
            // heading depth stays visible, and an aligned indent keeps wrapped
            // rows under the text.
            let prefix = StyledSpan::new(
                format!("{} ", "#".repeat(*level as usize)),
                SpanStyle::role(Role::Marker),
            );
            let indent = StyledSpan::new(
                " ".repeat(prefix.text.chars().count()),
                SpanStyle::role(Role::Marker),
            );
            out.push(wrap_marked(prefix, run_spans(runs, Role::Heading), indent));
            out.push(blank());
        }
        SceneNode::CodeBlock { language, text } => {
            let fence = fence_for(text);
            let code = SpanStyle::role(Role::CodeBlock);
            // Fence and code content are verbatim: never word-wrapped.
            out.push(verbatim(StyledSpan::new(
                format!("{fence}{}", language.as_str()),
                code,
            )));
            for line in text.split('\n') {
                out.push(verbatim(StyledSpan::new(line, code)));
            }
            // split('\n') on a trailing-newline string yields a final empty
            // element; drop it so the fence is flush against the last code line.
            if text.ends_with('\n') {
                out.pop();
            }
            out.push(verbatim(StyledSpan::new(fence, code)));
            out.push(blank());
        }
        SceneNode::Quote { runs, attribution } => {
            // Wrapped rows repeat the `> ` marker.
            let marker = StyledSpan::new("> ", SpanStyle::role(Role::Quote));
            out.push(wrap_marked(
                marker.clone(),
                run_spans(runs, Role::Quote),
                marker.clone(),
            ));
            if let Some(attr) = attribution {
                out.push(wrap_marked(
                    StyledSpan::new("> -- ", SpanStyle::role(Role::Quote)),
                    run_spans(attr, Role::Quote),
                    marker,
                ));
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
                let indent = " ".repeat(bullet.chars().count());
                out.push(wrap_marked(
                    StyledSpan::new(bullet, SpanStyle::role(Role::Marker)),
                    run_spans(item, Role::Plain),
                    StyledSpan::new(indent, SpanStyle::role(Role::Marker)),
                ));
            }
            out.push(blank());
        }
        SceneNode::Divider => {
            out.push(wrap(vec![StyledSpan::new(
                "---",
                SpanStyle::role(Role::Marker),
            )]));
            out.push(blank());
        }
        SceneNode::Image { image } => {
            let marker = SpanStyle::role(Role::Marker);
            if image.alt.is_empty() {
                out.push(wrap(vec![StyledSpan::new("[image]", marker)]));
            } else {
                out.push(wrap(vec![StyledSpan::new(
                    format!("[image: {}]", image.alt),
                    marker,
                )]));
            }
            if let Some(caption) = &image.caption {
                out.push(wrap(vec![StyledSpan::new(format!("({caption})"), marker)]));
            }
            out.push(blank());
        }
        SceneNode::Link { label, link } => {
            let mut spans = run_spans(label, Role::Link);
            spans.push(StyledSpan::new(
                format!(" (-> {})", link_target(link)),
                SpanStyle::role(Role::Link),
            ));
            out.push(wrap(spans));
            out.push(blank());
        }
        SceneNode::SubmitForm {
            label,
            submit_to,
            fields,
            submit_label,
        } => {
            let mut head = run_spans(label, Role::Plain);
            head.push(StyledSpan::new(
                format!(" (form -> {})", submit_to.as_str()),
                SpanStyle::role(Role::Marker),
            ));
            out.push(wrap(head));
            for field in fields {
                let indent = StyledSpan::new("  ", SpanStyle::role(Role::Marker));
                out.push(wrap_marked(
                    indent.clone(),
                    vec![StyledSpan::new(
                        form_field(field),
                        SpanStyle::role(Role::Marker),
                    )],
                    indent,
                ));
            }
            out.push(wrap(vec![StyledSpan::new(
                format!("[{submit_label}]"),
                SpanStyle::role(Role::Marker),
            )]));
            out.push(blank());
        }
        SceneNode::Feedback { variant, runs } => {
            let mut spans = vec![StyledSpan::new(
                format!("[{}] ", feedback_variant(variant)),
                SpanStyle::role(Role::Marker),
            )];
            spans.extend(run_spans(runs, Role::Plain));
            out.push(wrap(spans));
            out.push(blank());
        }
        SceneNode::Note {
            variant,
            title,
            runs,
        } => {
            let mut head = vec![StyledSpan::new(
                format!("[{}]", note_variant(variant)),
                SpanStyle::role(Role::Marker),
            )];
            if let Some(t) = title {
                head.push(StyledSpan::new(
                    format!(" {t}"),
                    SpanStyle::role(Role::Marker),
                ));
            }
            out.push(wrap(head));
            out.push(wrap(run_spans(runs, Role::Plain)));
            out.push(blank());
        }
    }
}

/// A verbatim row holding a single span.
fn verbatim(span: StyledSpan) -> LogicalLine {
    LogicalLine::Verbatim(vec![span])
}

/// Lower inline runs into styled spans of the given `role`. Inline marks become
/// span attributes; link runs append a ` (-> target)` marker-styled span.
fn run_spans(runs: &[InlineRun], role: Role) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    for run in runs {
        match run {
            InlineRun::Text { text, style } => {
                spans.push(StyledSpan::new(
                    text.clone(),
                    SpanStyle::from_marks(*style, role),
                ));
            }
            InlineRun::Link { text, style, link } => {
                spans.push(StyledSpan::new(
                    text.clone(),
                    SpanStyle::from_marks(*style, Role::Link),
                ));
                spans.push(StyledSpan::new(
                    format!(" (-> {})", link_target(link)),
                    SpanStyle::role(Role::Link),
                ));
            }
        }
    }
    spans
}

fn link_target(link: &LinkRef) -> String {
    match link {
        // Paths, onion addresses, and slugs have restricted char classes; URLs
        // are carried verbatim too now that there is no marker syntax to forge.
        LinkRef::SameSite { path } => path.as_str().to_owned(),
        LinkRef::Entangled { address, path, .. } => {
            format!("{}{}", address.as_str(), path.as_str())
        }
        LinkRef::Carrier { url, .. } => url.clone(),
        LinkRef::Citation { url } => url.clone(),
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
    format!("[{}] {} = \"{}\"{}", kind, name.as_str(), label, req)
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

/// Word-wrap `body` to `width` columns, pushing each wrapped row into `out`.
///
/// `prefix` is a fixed leading marker placed verbatim at the start of the first
/// row (never split), and `continuation` is prepended to every row after the
/// first; the available wrap width is reduced by whichever leads the row. Words
/// are split on spaces, each keeps the style of the span it came from, and
/// adjacent words of the same style are coalesced into one span per row. An
/// empty `body` produces one row (blank, or just the prefix when present).
fn wrap_into(
    prefix: Option<&StyledSpan>,
    body: &[StyledSpan],
    continuation: Option<&StyledSpan>,
    width: usize,
    out: &mut Vec<StyledLine>,
) {
    // Flatten the body spans into (word, style) units, splitting on spaces.
    let mut words: Vec<(String, SpanStyle)> = Vec::new();
    for span in body {
        for word in span.text.split(' ') {
            words.push((word.to_owned(), span.style));
        }
    }

    let prefix_len = prefix.map_or(0, |p| p.text.chars().count());
    let cont_len = continuation.map_or(0, |c| c.text.chars().count());
    let first = out.len();
    // Width left for body words: reduced by the prefix on row 0, by the
    // continuation on later rows.
    let row_width = |row_index: usize| {
        let lead = if row_index == 0 { prefix_len } else { cont_len };
        width.saturating_sub(lead).max(1)
    };

    let mut row: Vec<(String, SpanStyle)> = Vec::new();
    let mut row_len = 0usize;
    for (word, style) in words {
        if word.is_empty() {
            // A leading/trailing/double space in the source produced an empty
            // unit; skip it rather than emit a spurious space.
            continue;
        }
        let word_len = word.chars().count();
        let w = row_width(out.len() - first);
        if row_len == 0 {
            push_word(&word, style, w, &mut row, &mut row_len, out);
        } else if row_len + 1 + word_len <= w {
            row.push((" ".to_owned(), style));
            row.push((word, style));
            row_len += 1 + word_len;
        } else {
            out.push(coalesce(std::mem::take(&mut row)));
            row_len = 0;
            let w = row_width(out.len() - first);
            push_word(&word, style, w, &mut row, &mut row_len, out);
        }
    }
    if !row.is_empty() || out.len() == first {
        out.push(coalesce(row));
    }

    // Lead each row: the prefix on the first, the continuation on the rest.
    if let Some(prefix) = prefix {
        if !prefix.text.is_empty() {
            out[first].insert(0, prefix.clone());
        }
    }
    if let Some(continuation) = continuation {
        if !continuation.text.is_empty() {
            for line in out.iter_mut().skip(first + 1) {
                line.insert(0, continuation.clone());
            }
        }
    }
}

/// Place `word` (with `style`) at the start of a fresh row, hard-breaking it
/// across rows if it is wider than `width`.
fn push_word(
    word: &str,
    style: SpanStyle,
    width: usize,
    row: &mut Vec<(String, SpanStyle)>,
    row_len: &mut usize,
    out: &mut Vec<StyledLine>,
) {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() <= width {
        row.push((word.to_owned(), style));
        *row_len = chars.len();
        return;
    }
    for chunk in chars.chunks(width) {
        let piece: String = chunk.iter().collect();
        if chunk.len() == width {
            out.push(vec![StyledSpan::new(piece, style)]);
        } else {
            *row = vec![(piece, style)];
            *row_len = chunk.len();
        }
    }
}

/// Merge a row's (word, style) units into a `StyledLine`, joining adjacent units
/// that share a style into a single span.
fn coalesce(units: Vec<(String, SpanStyle)>) -> StyledLine {
    let mut line: StyledLine = Vec::new();
    for (text, style) in units {
        match line.last_mut() {
            Some(last) if last.style == style => last.text.push_str(&text),
            _ => line.push(StyledSpan::new(text, style)),
        }
    }
    line
}
