//! A self-contained visual demo of the terminal viewer.
//!
//! Run it with:
//!
//! ```sh
//! cargo run --example demo
//! ```
//!
//! It builds a rich scene directly from the engine's public IR types (no
//! document file, no signing - this is a rendering demo) and opens the
//! interactive viewer on it. Scroll with `j`/`k` or the arrows, page with
//! `space`/`PageUp`, jump with `g`/`G`, and quit with `q`. Resize the terminal
//! to watch the content re-wrap, keeping quote/list/form continuation markers
//! aligned.

use entangled_core::types::{
    Carrier, EntangledPath, ImageMediaType, ImageSha256, OnionAddress, Slug,
};
use entangled_engine::{FormFieldView, InlineRun, LinkRef, Scene, SceneNode, TextStyle};

fn text(s: &str) -> InlineRun {
    InlineRun::Text {
        text: s.to_owned(),
        style: TextStyle::default(),
    }
}

fn styled(s: &str, style: TextStyle) -> InlineRun {
    InlineRun::Text {
        text: s.to_owned(),
        style,
    }
}

fn bold(s: &str) -> InlineRun {
    styled(
        s,
        TextStyle {
            bold: true,
            ..TextStyle::default()
        },
    )
}

fn italic(s: &str) -> InlineRun {
    styled(
        s,
        TextStyle {
            italic: true,
            ..TextStyle::default()
        },
    )
}

fn code(s: &str) -> InlineRun {
    styled(
        s,
        TextStyle {
            code: true,
            ..TextStyle::default()
        },
    )
}

fn p(s: &str) -> EntangledPath {
    EntangledPath::try_from(s).expect("valid path")
}

fn slug(s: &str) -> Slug {
    Slug::try_from(s).expect("valid slug")
}

const ONION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.onion";

fn demo_scene() -> Scene {
    Scene {
        nodes: vec![
            SceneNode::Heading {
                level: 1,
                runs: vec![text("Welcome to Entangled")],
            },
            SceneNode::Paragraph {
                runs: vec![
                    text("This is a "),
                    bold("signed"),
                    text(" document served over a carrier such as Tor v3. There is "),
                    italic("no JavaScript"),
                    text(", no HTML, and a deliberately closed grammar. Resize your terminal to watch this long paragraph re-wrap to the available width in real time."),
                ],
            },
            SceneNode::Heading {
                level: 2,
                runs: vec![text("Why it exists")],
            },
            SceneNode::List {
                ordered: true,
                items: vec![
                    vec![text("verify the publisher identity, not the address")],
                    vec![text("survive server compromise and origin rotation")],
                    vec![text("keep the reader attack surface tiny, with a "), code("closed block grammar")],
                ],
            },
            SceneNode::Quote {
                runs: vec![text(
                    "Trust the publisher key, not the carrier address. That is the whole idea, and it is what lets identity survive a hostile host.",
                )],
                attribution: Some(vec![text("the design notes")]),
            },
            SceneNode::CodeBlock {
                language: slug("rust"),
                text: "fn verify(doc: &Document) -> Result<(), Error> {\n    let key = doc.publisher_pubkey();\n    key.verify(doc.payload(), doc.sig())\n}".to_owned(),
            },
            SceneNode::Image {
                image: entangled_engine::ImageRef {
                    src: p("/img/trust-chain.png"),
                    sha256: ImageSha256::try_from(
                        "sha-256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    )
                    .expect("sha"),
                    media_type: ImageMediaType::Png,
                    width: 1024,
                    height: 512,
                    alt: "the K_publisher / K_origin / K_runtime trust chain".to_owned(),
                    caption: Some("Figure 1: three keys, three roles.".to_owned()),
                },
            },
            SceneNode::Note {
                variant: entangled_core::types::NoteVariant::Info,
                title: Some("Chrome".to_owned()),
                runs: vec![text(
                    "The yellow bar above is client chrome. A real client would show trust state and canary state there; this viewer is honest that it shows neither.",
                )],
            },
            SceneNode::Link {
                label: vec![text("Read the full specification")],
                link: LinkRef::Citation {
                    url: "https://github.com/samjanny/entangled".to_owned(),
                },
            },
            SceneNode::Link {
                label: vec![text("A cross-site Entangled link")],
                link: LinkRef::Entangled {
                    carrier: Carrier::TorV3,
                    address: OnionAddress::try_from(ONION).expect("onion"),
                    path: p("/elsewhere"),
                    expected_publisher_pubkey: None,
                },
            },
            SceneNode::Divider,
            SceneNode::SubmitForm {
                label: vec![text("Get in touch")],
                submit_to: p("/contact"),
                fields: vec![
                    FormFieldView::Text {
                        name: slug("name"),
                        label: "Your name".to_owned(),
                        required: true,
                        max_length: 100,
                    },
                    FormFieldView::Textarea {
                        name: slug("message"),
                        label: "Your message to the publisher".to_owned(),
                        required: true,
                        max_length: 4096,
                    },
                ],
                submit_label: "Send".to_owned(),
            },
            SceneNode::Feedback {
                variant: entangled_core::types::FeedbackVariant::Success,
                runs: vec![text("This feedback strip is publisher content, not chrome.")],
            },
        ],
    }
}

fn main() -> std::io::Result<()> {
    entangled_tui::viewer::run(&demo_scene())
}
