use anyhow::{anyhow, Context, Result};
use clap::Parser;
use colored::*;
use dirs::home_dir;
use std::fs::{self, File};
use std::io::Write;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
//use tera::{Context as TeraContext, Tera};

mod resolver;

// ---- Clap colored help styling ----
use clap::builder::styling::{AnsiColor, Effects, Styles};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Nebula: Vivado Project Setup Tool (Rust + Tera)",
    long_about = None,
    styles = cli_styles(),
    color = clap::ColorChoice::Always
)]
struct Args {
    /// Project name
    #[arg(long)]
    proj_name: String,

    /// Source directories containing SystemVerilog files (can be specified multiple times)
    #[arg(long = "source-dir", action = clap::ArgAction::Append)]
    source_dirs: Vec<String>,

    /// Build output directory
    #[arg(long)]
    target_dir: String,

    /// FPGA part number (e.g., xczu7ev-ffvc1156-2-e, xc7z020clg400-1)
    #[arg(long)]
    part: String,

    /// Top-level SystemVerilog file (enables auto-dependency resolution)
    #[arg(long)]
    top_file: Option<String>,

    /// ELF file for embedded processor projects
    #[arg(long)]
    elf_file: Option<String>,

    /// Run full compile flow (synthesis + implementation + bitstream)
    #[arg(long)]
    compile: bool,

    /// Open Vivado GUI after TCL execution
    #[arg(long)]
    open_gui: bool,

    /// Additional TCL files to include (e.g., block design files from /bd directory)
    #[arg(long = "include-tcl", action = clap::ArgAction::Append)]
    include_tcl_files: Vec<String>,

    /// Constraints files to include (e.g., XDC files for pin assignments and timing)
    #[arg(long = "constraints", action = clap::ArgAction::Append)]
    constraints_files: Vec<String>,

    /// Xilinx IP files to import (e.g., XCI files for clocking wizards, ILAs, VIOs)
    #[arg(long = "ip", action = clap::ArgAction::Append)]
    ip_files: Vec<String>,

    /// List detected files and exit (debug mode - don't run Vivado)
    #[arg(long)]
    list_files: bool,

    /// Initialize application template (Justfile) in target directory
    #[arg(long)]
    init_template: bool,
}

// Define color scheme for clap help output
fn cli_styles() -> Styles {
    Styles::styled()
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .header(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Magenta.on_default())
}

/// Expand `~` to the user’s home directory, and return a `PathBuf`
fn expand_path(p: &str) -> PathBuf {
    if let Some(stripped) = p.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(p)
}

fn tcl_brace<S: AsRef<str>>(s: S) -> String {
    format!("{{{}}}", s.as_ref())
}

/// Simple colored logging helpers
fn info(msg: &str) {
    println!("{}", format!("[INFO] {}", msg).green());
}
fn warn(msg: &str) {
    println!("{}", format!("[WARN] {}", msg).yellow());
}
#[allow(dead_code)]
fn error(msg: &str) {
    eprintln!("{}", format!("[ERROR] {}", msg).red());
}

struct VivadoBuild {
    args: Args,
    target_file: PathBuf,
    target_dir: PathBuf,
    dependency_result: Option<resolver::DependencyResult>,
}

impl VivadoBuild {
    fn new(args: Args) -> Result<Self> {
        // Expand and normalize target_dir (allow non-existent directories)
        let target_dir_input = expand_path(&args.target_dir);
        let target_dir = target_dir_input.canonicalize().unwrap_or(target_dir_input);

        // Always use project name for output TCL file
        let output_name = format!("{}.tcl", args.proj_name);
        let target_file = target_dir.join(output_name);

        // Create build_artifacts directory
        let artifacts_dir = target_dir.join("build_artifacts");
        fs::create_dir_all(&artifacts_dir)
            .with_context(|| format!("Failed to create artifacts dir {:?}", artifacts_dir))?;

        // Resolve the dependency tree via sv-tree if a top file is specified
        let dependency_result = if let Some(top_file) = &args.top_file {
            info("Resolving SystemVerilog dependencies via sv-tree...");

            let top = expand_path(top_file);
            let search_paths: Vec<PathBuf> = args.source_dirs.iter()
                .map(|d| expand_path(d))
                .collect();

            info(&format!(
                "Searching {} source director{}",
                search_paths.len(),
                if search_paths.len() == 1 { "y" } else { "ies" }
            ));

            match resolver::resolve(&top, &search_paths) {
                Ok(result) => {
                    info(&format!(
                        "Resolved {} source files ({} unresolved)",
                        result.files.len(),
                        result.unresolved.len()
                    ));
                    Some(result)
                }
                Err(e) => {
                    warn(&format!("Dependency resolution failed: {}", e));
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            args,
            target_file,
            target_dir,
            dependency_result,
        })
    }

    fn write_target(&self, rendered: &str) -> Result<()> {
        if let Some(parent) = self.target_file.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent dir for {:?}", self.target_file)
            })?;
        }
        let mut file = File::create(&self.target_file)?;
        file.write_all(rendered.as_bytes())?;
        info(&format!(
            "Target TCL file written to {}",
            self.target_file.display()
        ));
        Ok(())
    }

    fn render_template(&self) -> Result<String> {
        use tera::{Context as TeraContext, Tera};

    // Expand and validate all source directories
    let source_dirs: Vec<PathBuf> = self.args.source_dirs.iter()
        .map(|d| expand_path(d))
        .collect();

    // Fail fast if any source_dir doesn't exist
    for source_dir in &source_dirs {
        if !source_dir.exists() {
            return Err(anyhow!(
                "Source directory does not exist: {}",
                source_dir.display()
            ));
        }
    }

    info(&format!("Source directories: {} total", source_dirs.len()));
    for (idx, dir) in source_dirs.iter().enumerate() {
        info(&format!("  [{}] {}", idx + 1, dir.display()));
    }
    
    // Use universal template
    let template_source_path = include_str!("../templates/universal.tclx").to_string();


        // ELF (brace-quoted), or {} when not provided
        let elf_path_braced = if let Some(elf) = &self.args.elf_file {
            let elf_path = expand_path(elf);
            if !elf_path.exists() {
                warn(&format!(
                    "ELF file '{}' not found; MicroBlaze attachment may fail.",
                    elf_path.display()
                ));
            }
            tcl_brace(elf_path.to_string_lossy())
        } else {
            "{}".to_string() // empty but valid Tcl value
        };

        // Prepare Tera
        let mut tera = Tera::default();
        let template_name = "nebula_template.tclx";

        tera.add_raw_template(template_name, &template_source_path)
            .with_context(|| format!("Failed to load template {}", template_name))?;

        // Build Tera context with Tcl-safe (brace-quoted) paths
        let mut ctx = TeraContext::new();
        ctx.insert("proj_name", &self.args.proj_name);
        ctx.insert("target_dir", &tcl_brace(self.target_dir.to_string_lossy()));

        // For backward compatibility, set origin_dir to the first source directory
        let origin_dir = if !source_dirs.is_empty() {
            tcl_brace(source_dirs[0].to_string_lossy())
        } else {
            "{}".to_string()
        };
        ctx.insert("origin_dir", &origin_dir);

        // Add all source directories as a list
        let source_dirs_tcl: Vec<String> = source_dirs.iter()
            .map(|d| tcl_brace(d.to_string_lossy()))
            .collect();
        ctx.insert("source_dirs", &source_dirs_tcl);

        ctx.insert("part", &self.args.part);
        ctx.insert("elf_path_file", &elf_path_braced);
        ctx.insert("compile", &self.args.compile);
        ctx.insert("open_gui", &self.args.open_gui);
        
        // Add include TCL files (expand paths and make them Tcl-safe)
        let include_tcl_expanded: Vec<String> = self.args.include_tcl_files.iter()
            .map(|path| tcl_brace(expand_path(path).to_string_lossy()))
            .collect();
        ctx.insert("include_tcl_files", &include_tcl_expanded);
        
        // Add constraints files (expand paths and make them Tcl-safe)
        let constraints_expanded: Vec<String> = self.args.constraints_files.iter()
            .map(|path| tcl_brace(expand_path(path).to_string_lossy()))
            .collect();
        ctx.insert("constraints_files", &constraints_expanded);

        // Add IP files (expand paths and make them Tcl-safe)
        let ip_expanded: Vec<String> = self.args.ip_files.iter()
            .map(|path| tcl_brace(expand_path(path).to_string_lossy()))
            .collect();
        ctx.insert("ip_files", &ip_expanded);
        
        // Add dependency information if available
        if let Some(deps) = &self.dependency_result {
            let file_list = deps.get_file_list();
            ctx.insert("sv_files", &file_list);
            ctx.insert("sv_file_count", &file_list.len());

            ctx.insert("sv_module_count", &file_list.len());
            ctx.insert("sv_unresolved_count", &deps.unresolved.len());
        } else {
            // No dependency resolution - set defaults
            let empty_files: Vec<String> = vec![];
            ctx.insert("sv_files", &empty_files);
            ctx.insert("sv_file_count", &0);
            ctx.insert("sv_module_count", &0);
            ctx.insert("sv_unresolved_count", &0);
        }

        let rendered = tera
            .render(template_name, &ctx)
            .with_context(|| format!("Failed to render template {:?}", template_name))?;

        Ok(rendered)
    }

    fn run_vivado(&self) -> Result<()> {
        // Always run project setup / RTL elaboration via the generated TCL
        info("Starting RTL elaboration / project setup...");
        let status = Command::new("vivado")
            .args(["-mode", "batch", "-source"])
            .arg(&self.target_file)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(anyhow!("Vivado batch run failed"));
        }
        info("RTL elaboration complete.");

        // Full compile only if requested
        if self.args.compile {
            if let Some(elf) = &self.args.elf_file {
                let elf_file = expand_path(elf);
                if !elf_file.exists() {
                    warn(&format!(
                        "ELF file '{}' not found; MicroBlaze attachment skipped.",
                        elf_file.display()
                    ));
                }
            }
            info("Starting synthesis, implementation, and bitstream generation...");
            // NOTE: Your .tclx can contain {% if compile %} blocks to drive full flow
            info("Bitstream and debug files generation complete.");
        } else {
            info("--compile false: skipping synthesis, implementation, and bitstream.");
        }

        Ok(())
    }
}

fn create_justfile_template(args: &Args) -> Result<()> {
    let target_dir = expand_path(&args.target_dir);

    // Create target directory if it doesn't exist
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("Failed to create target directory {:?}", target_dir))?;

    let justfile_path = target_dir.join("Justfile");

    if justfile_path.exists() {
        return Err(anyhow!("Justfile already exists in target directory: {}", justfile_path.display()));
    }

    // Build source directory arguments
    let source_dir_args = if args.source_dirs.is_empty() {
        String::new()
    } else {
        args.source_dirs.iter()
            .map(|d| format!("--source-dir {}", d))
            .collect::<Vec<_>>()
            .join(" \\\n    ")
    };

    // For Justfile variables, join source_dirs with space
    let source_dirs_var = args.source_dirs.join(" ");

    let top_file_arg = args.top_file.as_ref().map(|f| format!("--top-file {}", f)).unwrap_or_default();
    let elf_file_arg = args.elf_file.as_ref().map(|f| format!("--elf-file {}", f)).unwrap_or_default();

    let include_tcl_args = if args.include_tcl_files.is_empty() {
        String::new()
    } else {
        args.include_tcl_files.iter()
            .map(|f| format!("--include-tcl {}", f))
            .collect::<Vec<_>>()
            .join(" \\\n    ")
    };

    let constraints_args = if args.constraints_files.is_empty() {
        String::new()
    } else {
        args.constraints_files.iter()
            .map(|f| format!("--constraints {}", f))
            .collect::<Vec<_>>()
            .join(" \\\n    ")
    };

    let ip_args = if args.ip_files.is_empty() {
        String::new()
    } else {
        args.ip_files.iter()
            .map(|f| format!("--ip {}", f))
            .collect::<Vec<_>>()
            .join(" \\\n    ")
    };

    let justfile_content = format!(r#"#--------------------------------------------------------------
# Project : Nebula (Vivado setup) Overlay
# Author  : Generated by Nebula CLI
#--------------------------------------------------------------
# Description:
# Justfile driving the nebula CLI (rust) to set up and/or open
# a Vivado project.
#
# Usage examples:
#   just params
#   just build
#   just build proj_name="CustomName" target_dir="~/custom/build"
#--------------------------------------------------------------

#============================
# Defaults (independent vars)
#============================
project_name     := "{proj_name}"
source_dirs      := "{source_dirs}"
target_dir       := "{target_dir}"
part             := "{part}"
top_file         := "{top_file}"
elf_file         := "{elf_file}"
compile          := "{compile}"
open_gui         := "{open_gui}"

default: info

#--------------------------------------------------------------
# CLI parameter echo
#--------------------------------------------------------------
params:
  @echo "**** Nebula CLI arguments ****"
  @echo
  @echo "--proj-name        " {{{{ project_name }}}}
  @echo "--source-dirs      " {{{{ source_dirs }}}}
  @echo "--target-dir       " {{{{ target_dir }}}}
  @echo "--part             " {{{{ part }}}}
  @echo "--top-file         " {{{{ top_file }}}}
  @echo "--elf-file         " {{{{ elf_file }}}}
  @echo "--compile          " {{{{ compile }}}}
  @echo "--open-gui         " {{{{ open_gui }}}}

#--------------------------------------------------------------
# System information
#--------------------------------------------------------------
info:
  #!/usr/bin/env python3
  import platform, os
  print("")
  print("==========================================")
  print("Justfile         : Nebula Vivado Overlay")
  print("------------------------------------------")
  print("Operating System : " + platform.system())
  print("Release          : " + platform.release())
  print("Version          : " + platform.version())
  print("==========================================")
  print("")
  print("cwd: {{}}".format(os.getcwd()))
  print("")
  print("Basic options are: ")
  print("  just build")
  print("  just list")
  print("  just params")
  print("  just clean")

#--------------------------------------------------------------
# Build the Vivado project
#--------------------------------------------------------------
build: info
  @echo "**** Starting nebula (setup vivado) ....."
  nebula {source_dir_lines} \
    --proj-name {{{{ project_name }}}} \
    --target-dir {{{{ target_dir }}}} \
    --part {{{{ part }}}}{top_file_line}{elf_file_line}{include_tcl_lines}{constraints_lines}{ip_lines}{compile_flag}{open_gui_flag}

#--------------------------------------------------------------
# List files that would be added to the project
#--------------------------------------------------------------
list:
  nebula {source_dir_lines} \
    --proj-name {{{{ project_name }}}} \
    --target-dir {{{{ target_dir }}}} \
    --part {{{{ part }}}}{top_file_line}{elf_file_line}{include_tcl_lines}{constraints_lines}{ip_lines} \
    --list-files

#--------------------------------------------------------------
# Clean build artifacts
#--------------------------------------------------------------
clean:
  @echo "Cleaning build artifacts..."
  rm -rf {{{{ target_dir }}}}/build_artifacts
  rm -f {{{{ target_dir }}}}/{{{{ project_name }}}}.tcl
  @echo "Clean complete."
"#,
        proj_name = args.proj_name,
        source_dirs = source_dirs_var,
        target_dir = args.target_dir,
        part = args.part,
        top_file = args.top_file.as_deref().unwrap_or(""),
        elf_file = args.elf_file.as_deref().unwrap_or(""),
        compile = if args.compile { "true" } else { "false" },
        open_gui = if args.open_gui { "true" } else { "false" },
        source_dir_lines = source_dir_args,
        top_file_line = if top_file_arg.is_empty() { String::new() } else { format!(" \\\n    {}", top_file_arg) },
        elf_file_line = if elf_file_arg.is_empty() { String::new() } else { format!(" \\\n    {}", elf_file_arg) },
        include_tcl_lines = if include_tcl_args.is_empty() { String::new() } else { format!(" \\\n    {}", include_tcl_args) },
        constraints_lines = if constraints_args.is_empty() { String::new() } else { format!(" \\\n    {}", constraints_args) },
        ip_lines = if ip_args.is_empty() { String::new() } else { format!(" \\\n    {}", ip_args) },
        compile_flag = if args.compile { " \\\n    --compile" } else { "" },
        open_gui_flag = if args.open_gui { " \\\n    --open-gui" } else { "" },
    );

    fs::write(&justfile_path, justfile_content)
        .with_context(|| format!("Failed to write Justfile to {:?}", justfile_path))?;

    info(&format!("Justfile template created at: {}", justfile_path.display()));
    info("You can now run 'just build' in that directory to build your project.");

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle --init-template mode
    if args.init_template {
        info(&format!(
            "Nebula: Initializing template for project '{}'",
            args.proj_name
        ));
        return create_justfile_template(&args);
    }

    info(&format!(
        "Nebula: Vivado build starting for project '{}'",
        args.proj_name
    ));
    info(&format!("Target Directory: {}", args.target_dir));

    let build = VivadoBuild::new(args)?;
    
    // Display dependency analysis results if available
    if let Some(deps) = &build.dependency_result {
        deps.print_table();
    }
    
    // If list-files mode, just show the files and exit
    if build.args.list_files {
        if let Some(deps) = &build.dependency_result {
            println!("\n# Files that would be added to Vivado project:");
            for file in deps.get_file_list() {
                println!("  {}", file);
            }
        }
        
        if !build.args.include_tcl_files.is_empty() {
            println!("\n# TCL files that would be sourced:");
            for tcl_file in &build.args.include_tcl_files {
                println!("  {}", expand_path(tcl_file).display());
            }
        }
        
        if !build.args.constraints_files.is_empty() {
            println!("\n# Constraints files that would be added:");
            for constraints_file in &build.args.constraints_files {
                println!("  {}", expand_path(constraints_file).display());
            }
        }
        
        info("List mode complete - no Vivado project generated.");
        return Ok(());
    }
    
    let rendered = build.render_template()?;
    build.write_target(&rendered)?;
    build.run_vivado()?;

    info("Nebula build process finished.");
    Ok(())
}
