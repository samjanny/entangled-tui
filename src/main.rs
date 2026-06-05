//! `entangled-tui` binary: CLI glue.
//!
//! Loads a content document, lowers it to a scene with `entangled-engine`, and
//! hands it to the interactive viewer (`entangled_tui::viewer::run`). All
//! rendering and terminal logic lives in the library.
//!
//! # Verification boundary
//!
//! This viewer assumes its input is already verified, exactly as the engine
//! does. It runs the core's schema validation (pipeline Stages 2 through 5,
//! which re-applies every protocol invariant) but does NOT verify the Ed25519
//! signature: signature/trust verification is the job of a real client built
//! on `entangled-core`, not of a content viewer. The viewer is for inspecting
//! the rendered content of a document one already trusts.

use std::path::PathBuf;
use std::process::ExitCode;

use entangled_core::validation::parse_and_validate_content;
use entangled_engine::Scene;

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

    match entangled_tui::viewer::run(&scene) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Read a content document from `path` and lower it to a scene.
///
/// Uses the core's `parse_and_validate_content` (pipeline Stages 2 through 5:
/// byte cap, UTF-8, JSON limits, kind discrimination, and closed-schema
/// validation), which re-applies every protocol invariant but does NOT verify
/// the signature - matching this viewer's verification boundary. The wire JSON
/// carries a top-level `kind` discriminator that the bare `ContentDocument`
/// type does not accept on its own; the core's parser strips it via the
/// `Document` enum, which is why we go through this entry point rather than
/// deserializing `ContentDocument` directly.
fn load_scene(path: &std::path::Path) -> Result<Scene, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let doc = parse_and_validate_content(&bytes)
        .map_err(|d| format!("invalid content document: {d:?}"))?;
    Ok(Scene::from_content(&doc))
}
