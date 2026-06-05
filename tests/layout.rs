//! Golden tests for the pure layout and viewport logic.
//!
//! Scenes are built directly from the public engine IR types, laid out at a
//! fixed width, and asserted line by line. The viewport tests exercise scroll
//! arithmetic. No terminal is involved.

use entangled_core::types::Slug;
use entangled_engine::{InlineRun, LinkRef, Scene, SceneNode, TextStyle};
use entangled_tui::{lay_out, App};

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

#[test]
fn paragraph_wraps_to_width() {
    let scene = Scene {
        nodes: vec![para("one two three four five")],
    };
    // Width 9 columns: greedy word wrap.
    let lines = lay_out(&scene, 9);
    assert_eq!(
        lines,
        vec![
            "one two".to_owned(), // 7 <= 9; "+three" would be 13
            "three".to_owned(),   // 5; "+four" would be 10 > 9
            "four five".to_owned(),
            String::new(), // block separator
        ]
    );
}

#[test]
fn overlong_word_is_hard_broken() {
    let scene = Scene {
        nodes: vec![para("abcdefghijkl")], // 12 chars, width 5
    };
    let lines = lay_out(&scene, 5);
    assert_eq!(
        lines,
        vec![
            "abcde".to_owned(),
            "fghij".to_owned(),
            "kl".to_owned(),
            String::new(),
        ]
    );
}

#[test]
fn heading_and_marks_and_link_layout() {
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
    let lines = lay_out(&scene, 80);
    assert_eq!(
        lines,
        vec![
            "## Title".to_owned(),
            String::new(),
            "*hi* see (-> /x)".to_owned(),
            String::new(),
        ]
    );
}

#[test]
fn code_block_is_verbatim_and_not_wrapped() {
    let scene = Scene {
        nodes: vec![SceneNode::CodeBlock {
            language: Slug::try_from("text").unwrap(),
            // A long line that would wrap as prose stays intact as code.
            text: "a very long line of code that exceeds the narrow width".to_owned(),
        }],
    };
    let lines = lay_out(&scene, 10);
    assert_eq!(
        lines,
        vec![
            "```text".to_owned(),
            "a very long line of code that exceeds the narrow width".to_owned(),
            "```".to_owned(),
            String::new(),
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
    let lines = lay_out(&scene, 80);
    assert_eq!(
        lines,
        vec![
            "````text".to_owned(), // four backticks: longer than the embedded run
            "x".to_owned(),
            "```".to_owned(),
            "forged".to_owned(),
            "````".to_owned(),
            String::new(),
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
    // Each paragraph is "lineN" + a blank separator => 2 lines per node.
    let scene = many_lines_scene(5); // 10 laid-out lines
    let mut app = App::new(&scene, 80);
    assert_eq!(app.line_count(), 10);

    // Top of a 4-row viewport.
    assert_eq!(app.visible(4), &["line0", "", "line1", ""]);

    app.scroll_down(2, 4);
    assert_eq!(app.scroll(), 2);
    assert_eq!(app.visible(4), &["line1", "", "line2", ""]);
}

#[test]
fn scroll_clamps_at_bottom_and_top() {
    let scene = many_lines_scene(5); // 10 lines
    let mut app = App::new(&scene, 80);

    // Scrolling far down clamps to max_scroll = 10 - 4 = 6.
    app.scroll_down(1000, 4);
    assert_eq!(app.scroll(), 6);

    // Scrolling up past the top clamps to 0.
    app.scroll_up(1000);
    assert_eq!(app.scroll(), 0);

    // to_bottom jumps to the last full screen.
    app.to_bottom(4);
    assert_eq!(app.scroll(), 6);
    app.to_top();
    assert_eq!(app.scroll(), 0);
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
            "> trust the publisher".to_owned(),
            "> key not the carrier".to_owned(),
            "> address ever".to_owned(),
            String::new(),
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
    // Continuation rows are indented to the width of "1. " (three columns).
    assert_eq!(
        lay_out(&scene, 20),
        vec![
            "1. a fairly long".to_owned(),
            "   list item that".to_owned(),
            "   must wrap".to_owned(),
            "   somewhere".to_owned(),
            String::new(),
        ]
    );
}

#[test]
fn wrapped_form_field_keeps_indent() {
    use entangled_engine::FormFieldView;
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
            "F (form -> /s)".to_owned(),
            "[text] n = \"a very".to_owned(),
            "  long field label".to_owned(),
            "  that wraps\"".to_owned(),
            "  (required)".to_owned(),
            "[Go]".to_owned(),
            String::new(),
        ]
    );
}
