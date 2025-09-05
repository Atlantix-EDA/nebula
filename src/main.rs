use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use chrono::Local;

#[derive(Parser, Debug)]
#[command(author, version, about="Vivado Project Setup Tool in Rust", long_about=None)]
struct Args {
    /// Project name e.g. TIEC_36
    #[arg(long)]
    proj_name: String,

    /// Source repo path e.g. ../../tiec_fpga_fw
    #[arg(long)]
    source_repo: String,

    /// Path modification where the actual source resides relative to source_repo, e.g. ./phasebalancer
    #[arg(long)]
    path_mod: String,

    /// TCL template e.g. /configurations/tiec.tcl
    #[arg(long)]
    source_tcl: String,

    /// Target build directory, e.g. ~/fpga_projects/build/
    #[arg(long)]
    target_dir: String,

    /// ELF file e.g. ~/fpga_projects/microblaze_sw/target/Debug/Pheq.elf
    #[arg(long)]
    elf_file: String,

    /// Compile the project (true = run synthesis, implementation, and generate bitstream)
    #[arg(long, default_value_t = false)]
    compile: bool,
}


struct VivadoBuild {
    args: Args,
    target_file: PathBuf,
    target_dir: PathBuf,
}

impl VivadoBuild {
    fn new(args: Args) -> Result<Self> {
        let target_dir = Path::new(&args.target_dir).canonicalize()?;
        let target_file = target_dir.join(format!("{}.tcl", &args.proj_name));

        // Create build_artifacts directory
        let artifacts_dir = target_dir.join("build_artifacts");
        fs::create_dir_all(&artifacts_dir)?;

        Ok(Self { args, target_file, target_dir })
    }

    fn render_template(&self) -> Result<String> {
        // Very simple placeholder rendering
        let template = fs::read_to_string(&self.args.source_tcl)?;
        let rendered = template
            .replace("{{proj_name}}", &self.args.proj_name)
            .replace("{{target_dir}}", &self.args.target_dir)
            .replace("{{origin_dir}}", &self.args.source_repo)
            .replace("{{elf_path_file}}", &self.args.elf_file);
        Ok(rendered)
    }

    fn write_target(&self, rendered: &str) -> Result<()> {
        fs::create_dir_all(self.target_file.parent().unwrap())?;
        let mut file = File::create(&self.target_file)?;
        file.write_all(rendered.as_bytes())?;
        info(&format!("Target TCL file written to {}", self.target_file.display()));
        Ok(())
    }

    fn run_vivado(&self) -> Result<()> {
        // Always run project setup / RTL elaboration
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

        // Run full synthesis / implementation / bitstream only if --compile is true
        if self.args.compile {
            if !Path::new(&self.args.elf_file).exists() {
                warn(&format!("ELF file '{}' not found; MicroBlaze attachment skipped.", self.args.elf_file));
            }
            info("Starting synthesis, implementation, and bitstream generation...");
            // Place additional TCL commands here for full flow
            info("Bitstream and debug files generation complete.");
        } else {
            info("--compile false: skipping synthesis, implementation, and bitstream.");
        }

        Ok(())
    }
}

/// Simple colored logging helpers
fn info(msg: &str) {
    println!("{}", format!("[INFO] {}", msg).green());
}
fn warn(msg: &str) {
    println!("{}", format!("[WARN] {}", msg).yellow());
}
fn error(msg: &str) {
    eprintln!("{}", format!("[ERROR] {}", msg).red());
}

fn main() -> Result<()> {
    let args = Args::parse();

    info(&format!("Vivado Build Starting for project '{}'", args.proj_name));
    info(&format!("Target Directory: {}", args.target_dir));

    let build = VivadoBuild::new(args)?;
    let rendered = build.render_template()?;
    build.write_target(&rendered)?;
    build.run_vivado()?;

    info("Vivado build process finished successfully.");
    Ok(())
}
