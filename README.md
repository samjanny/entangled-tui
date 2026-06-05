# entangled-tui

[![CI](https://github.com/samjanny/entangled-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/samjanny/entangled-tui/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-orange.svg)](#building-and-testing)

A terminal content viewer for the Entangled v1.0 protocol. It renders a
**verified** document's scene IR (from [`entangled-engine`](https://github.com/samjanny/entangled-engine))
in a scrollable terminal view.

The protocol specification lives at [github.com/samjanny/entangled](https://github.com/samjanny/entangled).

```text
entangled-core      entangled-engine        entangled-tui
ContentDocument  -> Scene (toolkit-neutral) -> ratatui terminal view
(verified)          pure lowering              scroll, wrap, resize
```

## Not a conforming client

A conforming Entangled client (section 10) must present **chrome** - publisher
trust state, canary state, the PIP, and security warnings - in a persistent,
client-controlled region kept strictly separate from publisher content. That
chrome is security-critical and depends on the section 10 Stage 7 trust-state
machine and on canary resolution, which the scene IR does not carry and which no
crate in this workspace implements yet.

This viewer therefore renders the **content area only**, and shows a static,
honest chrome label stating that it is a content viewer and not a conforming
client - rather than a trust/canary indicator it has no basis to assert. A false
"verified" badge would be worse than none. When the trust-state layer exists,
this viewer can grow a real chrome; until then it does not pretend.

It also does not verify signatures: it runs the core's schema validation
(re-applying every protocol invariant) and renders the result. Signature and
trust verification belong to a real client built on `entangled-core`; this
viewer is for inspecting the rendered content of a document one already trusts.

## Rendering

The viewer applies real terminal styling, not a markup dialect: inline marks
(bold/italic/strikethrough, and inline code) render as terminal attributes and
colors with no delimiter characters, and headings, quotes, links, and code are
distinguished by color. A few structural markers are kept as text because the
terminal has no styling that conveys them: the `> ` quote prefix, list bullets,
`[image: alt]`, and the `(-> target)` / `(form -> path)` annotations. The pure
layout layer (`layout`) carries this as toolkit-neutral styled spans; the viewer
maps them to colors, so the layout stays testable as data.

Citation and carrier link URLs are shown defanged (`https://` -> `hxxps://`):
section 03 forbids the client from navigating to those clearnet/carrier targets
automatically, and a terminal that auto-linkifies a raw URL would turn it into a
one-click navigation. Defanging keeps the URL readable and copyable without the
emulator treating it as a clickable link.

## Structure

- `layout` (library): pure `Scene` -> width-wrapped, toolkit-neutral styled
  lines (`StyledLine`/`StyledSpan` carrying inline attributes and a semantic
  `Role`). Prose is word-wrapped; code blocks are verbatim with an adaptive
  fence. Color-free and testable, no I/O.
- `app` (library): viewer state (lines + scroll viewport). Pure arithmetic, no
  I/O.
- `viewer` (library): the crossterm/ratatui event-loop and draw shell over the
  two (`viewer::run`); the only part that touches the terminal.
- the `entangled-tui` binary: CLI glue that loads a document and hands the scene
  to `viewer::run`.

## Try it

The quickest way to see the viewer is the bundled demo, which builds a rich
scene in memory and opens it - no document file needed:

```sh
cargo run --example demo
```

To view a real document, pass a content-document JSON to the binary:

```sh
entangled-tui <content-document.json>
```

Keys: `j`/`k` or arrows to scroll, `space`/`PageDown` and `PageUp` to page,
`g`/`G` (or `Home`/`End`) to jump, `q`/`Esc` to quit. Resize the terminal to
watch the content re-wrap.

## Building and testing

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The pure layout and viewport logic is covered by golden tests; the terminal
shell is thin glue and is exercised by running the binary or `cargo run
--example demo`.

## License

Dual-licensed under either of:

- MIT License
- Apache License, Version 2.0

at your option.
