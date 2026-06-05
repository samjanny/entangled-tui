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

It also does not verify signatures: it deserializes a content document (which
re-applies the core type invariants) and renders it. Signature and trust
verification belong to a real client built on `entangled-core`; this viewer is
for inspecting the rendered content of a document one already trusts.

## Structure

- `layout` (library): pure `Scene` -> width-wrapped display lines. Prose is
  word-wrapped; code blocks are verbatim with an adaptive fence. Testable, no
  I/O.
- `app` (library): viewer state (lines + scroll viewport). Pure arithmetic, no
  I/O.
- the `entangled-tui` binary: the thin crossterm/ratatui event-loop and draw
  shell over the two.

## Usage

```sh
entangled-tui <content-document.json>
```

Keys: `j`/`k` or arrows to scroll, `space`/`PageDown` and `PageUp` to page,
`g`/`G` (or `Home`/`End`) to jump, `q`/`Esc` to quit.

## Building and testing

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The pure layout and viewport logic is covered by golden tests; the terminal
shell is thin glue and is exercised by running the binary.

## License

Dual-licensed under either of:

- MIT License
- Apache License, Version 2.0

at your option.
