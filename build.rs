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
    println!("cargo:rerun-if-changed=src/input.css");
    println!("cargo:rerun-if-changed=tailwind.config.js");
    println!("cargo:rerun-if-changed=templates/");

    // Build Tailwind CSS
    build_tailwind();

    // Hash static assets for cache busting
    let js_hash = hash_file(Path::new("static/js/card-interactions.js"));
    let css_hash = hash_file(Path::new("static/css/styles.css"));

    // Write generated code to OUT_DIR
    let out_dir = std::env::var("OUT_DIR").unwrap();
    fs::write(
        Path::new(&out_dir).join("asset_hashes.rs"),
        format!(
            r#"/// Hash of card-interactions.js for cache busting
pub const CARD_INTERACTIONS_JS_HASH: &str = "{}";
/// Hash of styles.css for cache busting
pub const STYLES_CSS_HASH: &str = "{}";"#,
            js_hash, css_hash
        ),
    )
    .unwrap();
}
