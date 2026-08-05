//! Dependency resolution via the `sv-tree` library.
//!
//! Nebula does not parse SystemVerilog itself. It links the
//! [`sv_tree`](https://github.com/Atlantix-EDA/sv-tree) crate, which uses
//! `sv-parser` for full IEEE-1800 parsing and walks the design hierarchy
//! (module instantiations and package imports) to produce the exact set of
//! source files a build needs. Resolution runs in-process, so nebula remains a
//! single self-contained binary.

use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};

/// Resolved dependency set for a top module.
pub struct DependencyResult {
    /// Required source files, sorted.
    pub files: Vec<String>,
    /// Names sv-tree could not locate in the search paths — Xilinx primitives,
    /// vendor IP, or block-design wrappers generated outside the RTL tree.
    /// Reported, not silently dropped, so the user can supply them explicitly.
    pub unresolved: Vec<String>,
}

impl DependencyResult {
    pub fn get_file_list(&self) -> Vec<String> {
        self.files.clone()
    }

    pub fn print_table(&self) {
        println!("\n# Dependency resolution (sv-tree)");
        println!("  required files: {}", self.files.len());
        for f in &self.files {
            println!("    {}", f);
        }
        if !self.unresolved.is_empty() {
            println!(
                "  unresolved ({}) — supply as IP / primitive / block-design wrapper:",
                self.unresolved.len()
            );
            for u in &self.unresolved {
                println!("    {}", u);
            }
        }
    }
}

/// Resolve the full source-file set for `top_file`, searching `search_dirs`.
pub fn resolve(top_file: &Path, search_dirs: &[PathBuf]) -> Result<DependencyResult> {
    let top_files = [top_file.to_path_buf()];

    let result = sv_tree::resolve(&top_files, search_dirs)
        .map_err(|e| anyhow!("sv-tree dependency resolution failed: {}", e))?;

    Ok(DependencyResult {
        files: result
            .files
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        unresolved: result.unresolved,
    })
}
