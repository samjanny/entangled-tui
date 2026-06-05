//! Terminal content viewer for the Entangled v1.0 protocol.
//!
//! This crate renders a **verified** document's scene IR
//! ([`entangled_engine::Scene`]) in a terminal. It is the second real consumer
//! of the IR (after the engine's plain-text renderer), and the first
//! interactive one: it exercises wrapping, a scroll viewport, and live resize.
//!
//! # Not a conforming client
//!
//! A conforming Entangled client (section 10) must present chrome - publisher
//! trust state, canary state, the PIP, and security warnings - in a persistent,
//! client-controlled region, kept strictly separate from publisher content.
//! That chrome is security-critical and depends on the section 10 Stage 7
//! trust-state machine and on canary resolution, which the scene IR does not
//! carry and which no crate in this workspace implements yet.
//!
//! This viewer therefore renders the **content area only**. It shows a static,
//! honest chrome label stating that it is a content viewer and not a conforming
//! client, rather than a trust/canary indicator it has no basis to assert - a
//! false "verified" badge would be worse than none. When the trust-state layer
//! exists, this viewer can grow a real chrome; until then it does not pretend.
//!
//! # Structure
//!
//! - [`layout`]: pure `Scene` -> width-wrapped display lines. Testable, no I/O.
//! - [`app`]: viewer state (lines + scroll viewport). Pure, no I/O.
//! - [`viewer`]: the crossterm/ratatui event-loop and draw shell over the two
//!   ([`viewer::run`]). The one part that touches the terminal.
//! - the `entangled-tui` binary (`main.rs`): CLI glue that loads a document and
//!   hands the scene to [`viewer::run`].

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod app;
pub mod layout;
pub mod viewer;

pub use app::App;
pub use layout::{lay_out, Role, SpanStyle, StyledLine, StyledSpan};

/// The static chrome label shown by the viewer (see the crate docs): it states
/// plainly that this is a content viewer, not a conforming client.
pub const CHROME_LABEL: &str =
    "entangled-tui - content viewer (NOT a conforming client: no trust/canary chrome)";
