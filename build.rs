use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;

fn hash_file(path: &Path) -> String {
    let content = fs::read(path).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())[..8].to_string()
}

/// Build the offline-srs WASM module (only when rebuild-wasm feature is enabled)
#[cfg(feature = "rebuild-wasm")]
fn build_wasm() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_dir = Path::new(&manifest_dir).join("crates/offline-srs");
    let output_dir = Path::new(&manifest_dir).join("static/wasm");

    // Check if wasm-pack is installed
    let wasm_pack_check = Command::new("wasm-pack").arg("--version").output();

    match wasm_pack_check {
        Ok(output) if output.status.success() => {}
        _ => {
            eprintln!("Warning: 'wasm-pack' not found, skipping WASM build");
            eprintln!("Install with: cargo install wasm-pack");
            return;
        }
    }

    // Build WASM module
    eprintln!("Building offline-srs WASM module...");
    let status = Command::new("wasm-pack")
        .current_dir(&crate_dir)
        .args(["build", "--target", "web", "--release"])
        .status();

    match status {
        Ok(s) if s.success() => {
            // Copy output files to static/wasm/
            fs::create_dir_all(&output_dir).ok();
            let pkg_dir = crate_dir.join("pkg");

            for file in ["offline_srs_bg.wasm", "offline_srs.js"] {
                let src = pkg_dir.join(file);
                let dst = output_dir.join(file);
                if src.exists() {
                    fs::copy(&src, &dst).ok();
                }
            }
            eprintln!("WASM build complete: {:?}", output_dir);
        }
        Ok(s) => {
            eprintln!("wasm-pack failed with status: {}", s);
        }
        Err(e) => {
            eprintln!("Failed to run wasm-pack: {}", e);
        }
    }
}

fn build_tailwind() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let css_path = Path::new(&manifest_dir).join("static/css/styles.css");

    let status = Command::new("tailwindcss")
        .current_dir(&manifest_dir)
        .args(["-i", "src/input.css", "-o", "static/css/styles.css", "--minify"])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("Tailwind CLI exited with status: {}", s);
            std::process::exit(1);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Allow builds without tailwindcss if CSS already exists (e.g., on deploy target)
            if css_path.exists() {
                eprintln!("Warning: 'tailwindcss' not found, using existing CSS");
                return;
            }
            eprintln!("Error: 'tailwindcss' not found in PATH");
            eprintln!();
            eprintln!("Install the standalone Tailwind CSS v4 binary:");
            eprintln!("  1. Download from: https://github.com/tailwindlabs/tailwindcss/releases");
            eprintln!("  2. Place in PATH (e.g., ~/.local/bin/tailwindcss)");
            eprintln!("  3. Make executable: chmod +x ~/.local/bin/tailwindcss");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to run Tailwind CLI: {}", e);
            std::process::exit(1);
        }
    }
}

fn main() {
    // Re-run build script if relevant files change
    println!("cargo:rerun-if-changed=static/js/card-interactions.js");
    println!("cargo:rerun-if-changed=static/js/sw-register.js");
    println!("cargo:rerun-if-changed=static/js/offline-storage.js");
    println!("cargo:rerun-if-changed=static/js/offline-sync.js");
    println!("cargo:rerun-if-changed=static/js/offline-study.js");
    println!("cargo:rerun-if-changed=static/js/vocabulary-search.js");
    println!("cargo:rerun-if-changed=static/sw.js");
    println!("cargo:rerun-if-changed=src/input.css");
    println!("cargo:rerun-if-changed=tailwind.config.js");
    println!("cargo:rerun-if-changed=templates/");

    // WASM source tracking (only matters when rebuild-wasm feature is enabled)
    #[cfg(feature = "rebuild-wasm")]
    println!("cargo:rerun-if-changed=crates/offline-srs/src/");

    // Build WASM module (only when feature is enabled)
    #[cfg(feature = "rebuild-wasm")]
    build_wasm();

    // Build Tailwind CSS
    build_tailwind();

    // Hash static assets for cache busting
    let js_hash = hash_file(Path::new("static/js/card-interactions.js"));
    let css_hash = hash_file(Path::new("static/css/styles.css"));
    let sw_register_hash = hash_file(Path::new("static/js/sw-register.js"));
    let sw_hash = hash_file(Path::new("static/sw.js"));
    let offline_storage_hash = hash_file(Path::new("static/js/offline-storage.js"));
    let offline_sync_hash = hash_file(Path::new("static/js/offline-sync.js"));
    let offline_study_hash = hash_file(Path::new("static/js/offline-study.js"));
    let vocabulary_search_hash = hash_file(Path::new("static/js/vocabulary-search.js"));

    // Write generated code to OUT_DIR
    let out_dir = std::env::var("OUT_DIR").unwrap();
    fs::write(
        Path::new(&out_dir).join("asset_hashes.rs"),
        format!(
            r#"/// Hash of card-interactions.js for cache busting
pub const CARD_INTERACTIONS_JS_HASH: &str = "{}";
/// Hash of styles.css for cache busting
pub const STYLES_CSS_HASH: &str = "{}";
/// Hash of sw-register.js for cache busting
pub const SW_REGISTER_JS_HASH: &str = "{}";
/// Hash of sw.js for cache busting
pub const SW_JS_HASH: &str = "{}";
/// Hash of offline-storage.js for cache busting
pub const OFFLINE_STORAGE_JS_HASH: &str = "{}";
/// Hash of offline-sync.js for cache busting
pub const OFFLINE_SYNC_JS_HASH: &str = "{}";
/// Hash of offline-study.js for cache busting
pub const OFFLINE_STUDY_JS_HASH: &str = "{}";
/// Hash of vocabulary-search.js for cache busting
pub const VOCABULARY_SEARCH_JS_HASH: &str = "{}";"#,
            js_hash, css_hash, sw_register_hash, sw_hash, offline_storage_hash, offline_sync_hash, offline_study_hash, vocabulary_search_hash
        ),
    )
    .unwrap();
}
