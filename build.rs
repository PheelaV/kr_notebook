use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

fn hash_file(path: &Path) -> String {
    let content = fs::read(path).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())[..8].to_string()
}

fn main() {
    // Re-run build script if static files change
    println!("cargo:rerun-if-changed=static/js/card-interactions.js");

    let js_hash = hash_file(Path::new("static/js/card-interactions.js"));

    // Write generated code to OUT_DIR
    let out_dir = std::env::var("OUT_DIR").unwrap();
    fs::write(
        Path::new(&out_dir).join("asset_hashes.rs"),
        format!(
            r#"/// Hash of card-interactions.js for cache busting
pub const CARD_INTERACTIONS_JS_HASH: &str = "{}";"#,
            js_hash
        ),
    )
    .unwrap();
}
