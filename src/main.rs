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
    /// Project name e.g. LowPassFilter or AudioEqualizer, etc.
    #[arg(long)]
    proj_name: String,

    /// Source repo path e.g. ../../fpga_projects/lowpassfilter/
    #[arg(long)]
    source_repo: String,

    /// Path modification relative to source_repo (e.g. . or ./subdir/)
    #[arg(long, default_value = "hw")]
    path_mod: String,

    /// TCLX template (extended TCL), e.g. /configurations/design.tclx
    #[arg(long = "source_tclx")]
    source_tclx: String,

    /// Target build directory, e.g. ~/fpga_projects/build/
    #[arg(long)]
    target_dir: String,

    /// ELF file e.g. ~/fpga_projects/softcore_mcu/target/Debug/Project.elf
    #[arg(long)]
    elf_file: Option<String>,

    /// Compile the project (true = synthesis, implementation, bitstream)
    #[arg(long, default_value_t = false)]
    compile: bool,

    /// Open the gui after running the tcl script (optional)
    #[arg(long, default_value_t = false)]
    open_gui: bool,
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
}

impl VivadoBuild {
    fn new(args: Args) -> Result<Self> {
        // Expand and normalize target_dir (allow non-existent directories)
        let target_dir_input = expand_path(&args.target_dir);
        let target_dir = target_dir_input.canonicalize().unwrap_or(target_dir_input);

        // Derive target filename from source_tclx, stripping .tclx -> .tcl
        let source_tclx = PathBuf::from(&args.source_tclx);
        let input_name = source_tclx
            .file_name()
            .ok_or_else(|| anyhow!("Invalid source_tclx path: {:?}", source_tclx))?;
        let mut output_name = input_name.to_string_lossy().to_string();

        if output_name.ends_with(".tclx") {
            // design.tclx → design.tcl
            output_name = output_name.trim_end_matches('x').to_string();
        } else {
            // Warn if the input is not a .tclx template, with suggestion
            let suggestion = input_name
                .to_string_lossy()
                .to_string()
                .strip_suffix(".tcl")
                .map(|s| format!("{s}.tclx"))
                .unwrap_or_else(|| format!("{}.tclx", input_name.to_string_lossy()));
            warn(&format!(
                "Template file '{}' does not use .tclx extension.\n       \
                 Rename it to '{}' and rerun Nebula to follow best practices.",
                input_name.to_string_lossy(),
                suggestion
            ));
        }

        let target_file = target_dir.join(output_name);

        // Create build_artifacts directory
        let artifacts_dir = target_dir.join("build_artifacts");
        fs::create_dir_all(&artifacts_dir)
            .with_context(|| format!("Failed to create artifacts dir {:?}", artifacts_dir))?;

        Ok(Self {
            args,
            target_file,
            target_dir,
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

    // Expand base paths
    let source_repo = expand_path(&self.args.source_repo);
    let source_tclx_path = expand_path(&self.args.source_tclx);

    // origin_dir = source_repo[/path_mod], default "hw"
    let path_mod_str = self.args.path_mod.trim();
    let origin_dir = if path_mod_str == "." || path_mod_str.is_empty() {
        source_repo.clone()
    } else {
        source_repo.join(path_mod_str)
    };

    // Fail fast if origin_dir doesn't exist
    if !origin_dir.exists() {
        return Err(anyhow!(
            "Resolved origin_dir does not exist: {} (source_repo={}, path_mod={})",
            origin_dir.display(),
            source_repo.display(),
            path_mod_str
        ));
    }

    // Helpful log
    info(&format!("Origin dir resolved to: {}", origin_dir.display()));

        // // origin_dir = source_repo[/path_mod]
        // let origin_dir = if self.args.path_mod == "." {
        //     source_repo.clone()
        // } else {
        //     source_repo.join(std::path::PathBuf::from(&self.args.path_mod))
        // };

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
        let template_name = std::path::Path::new(&self.args.source_tclx)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid source_tclx filename"))?;

        tera.add_template_file(&source_tclx_path, Some(template_name))
            .with_context(|| format!("Failed to load template {:?}", source_tclx_path))?;

        // Build Tera context with Tcl-safe (brace-quoted) paths
        let mut ctx = TeraContext::new();
        ctx.insert("proj_name", &self.args.proj_name);
        ctx.insert("target_dir", &tcl_brace(self.target_dir.to_string_lossy()));
        ctx.insert("origin_dir", &tcl_brace(origin_dir.to_string_lossy()));
        ctx.insert("elf_path_file", &elf_path_braced);
        ctx.insert("compile", &self.args.compile);
        ctx.insert("open_gui", &self.args.open_gui);

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

fn main() -> Result<()> {
    let args = Args::parse();

    info(&format!(
        "Nebula: Vivado build starting for project '{}'",
        args.proj_name
    ));
    info(&format!("Target Directory: {}", args.target_dir));

    let build = VivadoBuild::new(args)?;
    let rendered = build.render_template()?;
    build.write_target(&rendered)?;
    build.run_vivado()?;

    info("Nebula build process finished.");
    Ok(())
}
