//! Generator pack execution and management
//!
//! Generators are packs that contain scripts (typically scrapers) that produce
//! content like audio files. This module handles:
//! - Loading generator pack configurations
//! - Executing generator commands
//! - Routing output to appropriate locations based on scope

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::paths;

/// Generator execution result
#[derive(Debug)]
pub struct GeneratorResult {
    pub success: bool,
    pub output_path: PathBuf,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// Generator subcommand configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GeneratorSubcommand {
    /// Subcommand identifier (e.g., "lesson1")
    pub id: String,
    /// Command line arguments
    pub args: Vec<String>,
    /// Output subdirectory (e.g., "lesson1/")
    pub output: String,
}

/// Generator pack configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GeneratorConfig {
    /// Command to execute (e.g., "uv run kr-scraper")
    pub command: String,
    /// Available subcommands
    pub subcommands: Vec<GeneratorSubcommand>,
    /// Type of content produced (e.g., "audio")
    pub output_type: String,
}

/// Output scope for generator execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputScope {
    /// Output to shared location (data/content/generated/)
    Shared,
    /// Output to user's personal location (data/users/{username}/content/generated/)
    User,
}

/// Execute a generator subcommand
///
/// # Arguments
/// * `config` - Generator configuration from pack manifest
/// * `subcommand_id` - ID of the subcommand to run (e.g., "lesson1")
/// * `scope` - Where to output the generated content
/// * `username` - Username for user-scoped output (required if scope is User)
///
/// # Returns
/// GeneratorResult with execution status and output
pub fn execute_generator(
    config: &GeneratorConfig,
    subcommand_id: &str,
    scope: OutputScope,
    username: Option<&str>,
) -> Result<GeneratorResult, String> {
    // Find the subcommand
    let subcommand = config
        .subcommands
        .iter()
        .find(|s| s.id == subcommand_id)
        .ok_or_else(|| format!("Unknown subcommand: {}", subcommand_id))?;

    // Determine output directory based on scope
    let output_base = match scope {
        OutputScope::Shared => PathBuf::from(paths::SHARED_GENERATED_DIR).join("htsk"),
        OutputScope::User => {
            let user = username.ok_or("Username required for user-scoped output")?;
            PathBuf::from(paths::user_generated_dir(user)).join("htsk")
        }
    };

    let output_path = output_base.join(&subcommand.output.trim_end_matches('/'));

    // Create output directory
    std::fs::create_dir_all(&output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Parse command (handle "uv run kr-scraper" style commands)
    let command_parts: Vec<&str> = config.command.split_whitespace().collect();
    if command_parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let program = command_parts[0];
    let mut cmd = Command::new(program);

    // Add command arguments (e.g., "run kr-scraper")
    for part in &command_parts[1..] {
        cmd.arg(part);
    }

    // Add subcommand (e.g., "lesson1")
    cmd.arg(&subcommand.id);

    // Add subcommand args
    for arg in &subcommand.args {
        cmd.arg(arg);
    }

    // Add output directory
    cmd.arg("--output");
    cmd.arg(&output_path);

    // Set working directory to project root
    cmd.current_dir(paths::PY_SCRIPTS_DIR);

    // Capture output
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    tracing::info!(
        "Executing generator: {} {} --output {}",
        config.command,
        subcommand.id,
        output_path.display()
    );

    // Execute
    let output = cmd.output().map_err(|e| format!("Failed to execute command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(GeneratorResult {
        success: output.status.success(),
        output_path,
        stdout,
        stderr,
        exit_code: output.status.code(),
    })
}

/// List available generator packs
pub fn list_generators() -> Vec<GeneratorPackInfo> {
    let mut generators = Vec::new();

    // Check shared packs directory
    let shared_packs = Path::new(paths::SHARED_PACKS_DIR);
    if shared_packs.exists() {
        if let Ok(entries) = std::fs::read_dir(shared_packs) {
            for entry in entries.filter_map(|e| e.ok()) {
                let pack_path = entry.path();
                if let Some(info) = load_generator_pack_info(&pack_path) {
                    generators.push(info);
                }
            }
        }
    }

    generators
}

/// Generator pack metadata
#[derive(Debug, Clone)]
pub struct GeneratorPackInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub subcommands: Vec<String>,
    pub pack_path: PathBuf,
}

/// Load generator pack info from a pack directory
fn load_generator_pack_info(pack_path: &Path) -> Option<GeneratorPackInfo> {
    let manifest_path = pack_path.join("pack.json");
    if !manifest_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Only consider generator packs
    if manifest["type"].as_str() != Some("generator") {
        return None;
    }

    let generator = manifest.get("generator")?;
    let subcommands: Vec<String> = generator["subcommands"]
        .as_array()?
        .iter()
        .filter_map(|s| s["id"].as_str().map(String::from))
        .collect();

    Some(GeneratorPackInfo {
        id: manifest["id"].as_str()?.to_string(),
        name: manifest["name"].as_str()?.to_string(),
        description: manifest["description"].as_str().map(String::from),
        subcommands,
        pack_path: pack_path.to_path_buf(),
    })
}

/// Load generator config from a pack
pub fn load_generator_config(pack_path: &Path) -> Option<GeneratorConfig> {
    let manifest_path = pack_path.join("pack.json");
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&content).ok()?;

    if manifest["type"].as_str() != Some("generator") {
        return None;
    }

    let generator_value = manifest.get("generator")?;
    serde_json::from_value(generator_value.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_generators() {
        // Should not panic
        let generators = list_generators();
        assert!(generators.len() >= 0);
    }

    #[test]
    fn test_output_scope() {
        assert_eq!(OutputScope::Shared, OutputScope::Shared);
        assert_ne!(OutputScope::Shared, OutputScope::User);
    }
}
