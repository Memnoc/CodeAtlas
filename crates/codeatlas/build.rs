//! Embeds the dashboard's production build into the binary (ticket 09,
//! ADR-0002's single-binary story).
//!
//! `dashboard/dist/` is a build product of `npm run build` and is gitignored,
//! so this script owns the build story honestly:
//!
//! - dist missing, `node_modules` present → run `npm run build` (pure local
//!   Vite compilation; npm scripts fetch nothing, so this never touches the
//!   network — ADR-0006).
//! - dist missing, `node_modules` missing → fail the build with instructions;
//!   running `npm ci` on the developer's behalf would be a hidden network
//!   access, which ADR-0006 exists to forbid.
//! - dist present but older than the dashboard sources → rebuild when
//!   `node_modules` allows, otherwise warn and embed the stale build (an
//!   offline machine with a prebuilt dist must still compile).
//!
//! Embedding mechanism: this script generates `$OUT_DIR/embedded_assets.rs`,
//! a static table of (url path, MIME type, `include_bytes!`). Generating the
//! table here instead of pulling `include_dir`/`rust-embed` keeps the
//! dependency tree exactly as auditable as it was — zero new crates on the
//! serve path (ADR-0006's audit posture).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dashboard = manifest.join("../../dashboard").canonicalize().unwrap();
    let dist = dashboard.join("dist");

    // Rerun when the dashboard sources or the built output change. Directory
    // paths are watched recursively by cargo.
    for input in [
        "src",
        "index.html",
        "vite.config.ts",
        "package.json",
        "dist",
    ] {
        println!("cargo:rerun-if-changed={}", dashboard.join(input).display());
    }

    ensure_dist(&dashboard, &dist);

    let mut assets = Vec::new();
    collect(&dist, &dist, &mut assets);
    // Deterministic embedding order regardless of directory iteration order.
    assets.sort();

    let mut code = String::from(
        "/// One embedded dashboard file, addressable by its URL path.\n\
         pub struct Asset {\n\
         \x20   pub path: &'static str,\n\
         \x20   pub content_type: &'static str,\n\
         \x20   pub bytes: &'static [u8],\n\
         }\n\n\
         pub static ASSETS: &[Asset] = &[\n",
    );
    for rel in &assets {
        let abs = dist.join(rel);
        code.push_str(&format!(
            "    Asset {{ path: {rel:?}, content_type: {mime:?}, bytes: include_bytes!({abs:?}) }},\n",
            mime = content_type(rel),
            abs = abs.display().to_string(),
        ));
    }
    code.push_str("];\n");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("embedded_assets.rs");
    fs::write(out, code).unwrap();
}

/// Guarantees `dist/index.html` exists and is as fresh as the sources allow,
/// per the policy in the module comment.
fn ensure_dist(dashboard: &Path, dist: &Path) {
    let index = dist.join("index.html");
    let have_dist = index.exists();
    let have_node_modules = dashboard.join("node_modules").exists();
    let stale = have_dist && {
        let built = mtime(&index);
        ["src", "index.html", "vite.config.ts", "package.json"]
            .iter()
            .map(|input| newest_mtime(&dashboard.join(input)))
            .any(|changed| changed > built)
    };

    if have_dist && !stale {
        return;
    }
    if !have_node_modules {
        if have_dist {
            println!(
                "cargo:warning=dashboard/dist is older than dashboard sources and \
                 node_modules is missing; embedding the stale build. \
                 Run `npm ci && npm run build` in dashboard/ to refresh."
            );
            return;
        }
        panic!(
            "dashboard/dist is missing and dashboard/node_modules is not installed, \
             so the dashboard cannot be built to embed it. \
             Run `npm ci && npm run build` in dashboard/ first \
             (this build script never fetches anything itself)."
        );
    }
    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(dashboard)
        .status()
        .expect("failed to invoke npm — is Node installed?");
    assert!(status.success(), "`npm run build` failed in dashboard/");
}

fn collect(dist: &Path, dir: &Path, assets: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(dist, &path, assets);
        } else {
            let rel = path.strip_prefix(dist).unwrap();
            assets.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn mtime(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn newest_mtime(path: &Path) -> SystemTime {
    if !path.is_dir() {
        return mtime(path);
    }
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| newest_mtime(&e.path()))
                .max()
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// MIME by extension for everything Vite emits (and the browser needs to
/// render): html/js/css/json are the load-bearing ones; the rest keep any
/// future emitted asset honest.
fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
