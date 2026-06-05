//! Golden tests for the pure layout and viewport logic.
//!
//! Scenes are built directly from the public engine IR types, laid out at a
//! fixed width, and asserted as styled rows. Inline marks are carried as span
//! attributes (no delimiter characters); block roles are carried as a `Role`.
//! The viewport tests exercise scroll arithmetic. No terminal is involved.

use entangled_core::types::Slug;
use entangled_engine::{FormFieldView, InlineRun, LinkRef, Scene, SceneNode, TextStyle};
use entangled_tui::{lay_out, App, Role, SpanStyle, StyledLine, StyledSpan};

// --- construction helpers for the expected styled rows ---

/// A span with the given text and role, no inline attributes.
fn sp(text: &str, role: Role) -> StyledSpan {
    StyledSpan {
        text: text.to_owned(),
        style: SpanStyle {
            role,
            ..SpanStyle::default()
        },
    }
}

/// A bold span with the given text and role.
fn sp_bold(text: &str, role: Role) -> StyledSpan {
    StyledSpan {
        text: text.to_owned(),
        style: SpanStyle {
            role,
            bold: true,
            ..SpanStyle::default()
        },
    }
}

fn plain(t: &str) -> InlineRun {
    InlineRun::Text {
        text: t.to_owned(),
        style: TextStyle::default(),
    }
}

fn para(t: &str) -> SceneNode {
    SceneNode::Paragraph {
        runs: vec![plain(t)],
    }
}

/// An empty (blank separator) row.
fn nil() -> StyledLine {
    Vec::new()
}

#[test]
fn paragraph_wraps_to_width() {
    let scene = Scene {
        nodes: vec![para("one two three four five")],
    };
    // Width 9 columns: greedy word wrap, all Plain.
    assert_eq!(
        lay_out(&scene, 9),
        vec![
            vec![sp("one two", Role::Plain)],
            vec![sp("three", Role::Plain)],
            vec![sp("four five", Role::Plain)],
            nil(),
        ]
    );
}

#[test]
fn overlong_word_is_hard_broken() {
    let scene = Scene {
        nodes: vec![para("abcdefghijkl")], // 12 chars, width 5
    };
    assert_eq!(
        lay_out(&scene, 5),
        vec![
            vec![sp("abcde", Role::Plain)],
            vec![sp("fghij", Role::Plain)],
            vec![sp("kl", Role::Plain)],
            nil(),
        ]
    );
}

#[test]
fn heading_marker_and_inline_bold_carry_as_style() {
    let scene = Scene {
        nodes: vec![
            SceneNode::Heading {
                level: 2,
                runs: vec![plain("Title")],
            },
            SceneNode::Paragraph {
                runs: vec![
                    InlineRun::Text {
                        text: "hi".to_owned(),
                        style: TextStyle {
                            bold: true,
                            ..TextStyle::default()
                        },
                    },
                    InlineRun::Link {
                        text: " see".to_owned(),
                        style: TextStyle::default(),
                        link: LinkRef::SameSite {
                            path: entangled_core::types::EntangledPath::try_from("/x").unwrap(),
                        },
                    },
                ],
            },
        ],
    };
    // The `## ` prefix is a Marker; the heading text is Role::Heading. The bold
    // run keeps its text exactly (no `*`), carrying bold as an attribute.
    assert_eq!(
        lay_out(&scene, 80),
        vec![
            vec![sp("## ", Role::Marker), sp("Title", Role::Heading)],
            nil(),
            vec![sp_bold("hi", Role::Plain), sp(" see (-> /x)", Role::Link)],
            nil(),
        ]
    );
}

#[test]
fn inline_marks_emit_no_delimiter_characters() {
    // A run with every mark set must keep its text verbatim and carry the marks
    // as attributes - the old renderer would have wrapped it in `*/`~...~`/*`.
    let scene = Scene {
        nodes: vec![SceneNode::Paragraph {
            runs: vec![InlineRun::Text {
                text: "styled".to_owned(),
                style: TextStyle {
                    bold: true,
                    italic: true,
                    code: true,
                    strikethrough: true,
                },
            }],
        }],
    };
    let rows = lay_out(&scene, 80);
    assert_eq!(rows[0].len(), 1);
    let span = &rows[0][0];
    assert_eq!(span.text, "styled");
    assert!(span.style.bold && span.style.italic && span.style.code && span.style.strikethrough);
}

#[test]
fn code_block_is_verbatim_and_not_wrapped() {
    let scene = Scene {
        nodes: vec![SceneNode::CodeBlock {
            language: Slug::try_from("text").unwrap(),
            text: "a very long line of code that exceeds the narrow width".to_owned(),
        }],
    };
    assert_eq!(
        lay_out(&scene, 10),
        vec![
            vec![sp("```text", Role::CodeBlock)],
            vec![sp(
                "a very long line of code that exceeds the narrow width",
                Role::CodeBlock
            )],
            vec![sp("```", Role::CodeBlock)],
            nil(),
        ]
    );
}

#[test]
fn hostile_code_fence_does_not_break_out() {
    let scene = Scene {
        nodes: vec![SceneNode::CodeBlock {
            language: Slug::try_from("text").unwrap(),
            text: "x\n```\nforged".to_owned(),
        }],
    };
    // Four-backtick fence (longer than the embedded run of three).
    assert_eq!(
        lay_out(&scene, 80),
        vec![
            vec![sp("````text", Role::CodeBlock)],
            vec![sp("x", Role::CodeBlock)],
            vec![sp("```", Role::CodeBlock)],
            vec![sp("forged", Role::CodeBlock)],
            vec![sp("````", Role::CodeBlock)],
            nil(),
        ]
    );
}

#[test]
fn citation_and_carrier_urls_are_defanged() {
    // The viewer must not present a clickable clearnet/carrier URL (a terminal
    // would auto-linkify it into a one-click navigation, which section 03
    // forbids). The scheme is defanged so the emulator does not linkify it.
    let citation = Scene {
        nodes: vec![SceneNode::Link {
            label: vec![plain("ref")],
            link: LinkRef::Citation {
                url: "https://example.org/x".to_owned(),
            },
        }],
    };
    let row = &lay_out(&citation, 80)[0];
    let rendered: String = row.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(rendered, "ref (-> hxxps://example.org/x)");
    assert!(!rendered.contains("https://"));

    let carrier = Scene {
        nodes: vec![SceneNode::Link {
            label: vec![plain("svc")],
            link: LinkRef::Carrier {
                carrier: entangled_core::types::Carrier::TorV3,
                url: "http://example.onion/y".to_owned(),
            },
        }],
    };
    let row = &lay_out(&carrier, 80)[0];
    let rendered: String = row.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(rendered, "svc (-> hxxp://example.onion/y)");
    assert!(!rendered.contains("http://"));
}

// --- continuation prefixes on wrapped lines ---

#[test]
fn wrapped_quote_keeps_marker() {
    let scene = Scene {
        nodes: vec![SceneNode::Quote {
            runs: vec![plain(
                "trust the publisher key not the carrier address ever",
            )],
            attribution: None,
        }],
    };
    assert_eq!(
        lay_out(&scene, 24),
        vec![
            vec![
                sp("> ", Role::Quote),
                sp("trust the publisher", Role::Quote)
            ],
            vec![
                sp("> ", Role::Quote),
                sp("key not the carrier", Role::Quote)
            ],
            vec![sp("> ", Role::Quote), sp("address ever", Role::Quote)],
            nil(),
        ]
    );
}

#[test]
fn wrapped_list_item_aligns_under_text() {
    let scene = Scene {
        nodes: vec![SceneNode::List {
            ordered: true,
            items: vec![vec![plain(
                "a fairly long list item that must wrap somewhere",
            )]],
        }],
    };
    // The bullet is a Marker; continuation rows indent to its width (3 cols).
    assert_eq!(
        lay_out(&scene, 20),
        vec![
            vec![sp("1. ", Role::Marker), sp("a fairly long", Role::Plain)],
            vec![sp("   ", Role::Marker), sp("list item that", Role::Plain)],
            vec![sp("   ", Role::Marker), sp("must wrap", Role::Plain)],
            vec![sp("   ", Role::Marker), sp("somewhere", Role::Plain)],
            nil(),
        ]
    );
}

#[test]
fn wrapped_form_field_keeps_indent() {
    let scene = Scene {
        nodes: vec![SceneNode::SubmitForm {
            label: vec![plain("F")],
            submit_to: entangled_core::types::EntangledPath::try_from("/s").unwrap(),
            fields: vec![FormFieldView::Text {
                name: Slug::try_from("n").unwrap(),
                label: "a very long field label that wraps".to_owned(),
                required: true,
                max_length: 10,
            }],
            submit_label: "Go".to_owned(),
        }],
    };
    assert_eq!(
        lay_out(&scene, 22),
        vec![
            vec![sp("F", Role::Plain), sp(" (form -> /s)", Role::Marker)],
            vec![
                sp("  ", Role::Marker),
                sp("[text] n = \"a very", Role::Marker)
            ],
            vec![sp("  ", Role::Marker), sp("long field label", Role::Marker)],
            vec![sp("  ", Role::Marker), sp("that wraps\"", Role::Marker)],
            vec![sp("  ", Role::Marker), sp("(required)", Role::Marker)],
            vec![sp("[Go]", Role::Marker)],
            nil(),
        ]
    );
}

// --- viewport / scroll ---

fn many_lines_scene(n: usize) -> Scene {
    Scene {
        nodes: (0..n).map(|i| para(&format!("line{i}"))).collect(),
    }
}

#[test]
fn visible_slice_follows_scroll() {
    // Each paragraph is one row + a blank separator => 2 rows per node.
    let scene = many_lines_scene(5); // 10 laid-out rows
    let mut app = App::new(&scene, 80);
    assert_eq!(app.line_count(), 10);

    // Top of a 4-row viewport: line0, blank, line1, blank.
    let top = app.visible(4);
    assert_eq!(top.len(), 4);
    assert_eq!(top[0], vec![sp("line0", Role::Plain)]);
    assert_eq!(top[1], nil());
    assert_eq!(top[2], vec![sp("line1", Role::Plain)]);

    app.scroll_down(2, 4);
    assert_eq!(app.scroll(), 2);
    assert_eq!(app.visible(4)[0], vec![sp("line1", Role::Plain)]);
}

#[test]
fn scroll_clamps_at_bottom_and_top() {
    let scene = many_lines_scene(5); // 10 rows
    let mut app = App::new(&scene, 80);

    app.scroll_down(1000, 4);
    assert_eq!(app.scroll(), 6); // max_scroll = 10 - 4

    app.scroll_up(1000);
    assert_eq!(app.scroll(), 0);

    app.to_bottom(4);
    assert_eq!(app.scroll(), 6);
    app.to_top();
    assert_eq!(app.scroll(), 0);
}
