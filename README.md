# Nebula

Nebula is a template-driven build tool for FPGA projects, designed to simplify the process of generating reproducible Vivado TCL scripts. It operates on templates and filelists, giving you precise control over which RTL files, IP cores, and constraints are included in your builds. It doesn't get in your way with complex package management or dependency resolution, as it is a binary written in Rust with zero external dependencies.

[![Rust](https://img.shields.io/badge/rust-000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![SystemVerilog](https://img.shields.io/badge/SystemVerilog-IEEE--1800-blue?style=flat)](https://en.wikipedia.org/wiki/SystemVerilog)
[![Vivado](https://img.shields.io/badge/Vivado-TCL-orange?style=flat&logo=xilinx)](https://www.xilinx.com/products/design-tools/vivado.html)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat)](LICENSE)

**Table of Contents**
- [Principles](#principles)
- [Installation](#installation) 
- [Quick Start](#quick-start)
- [Template System](#template-system)
- [Filelist Format](#filelist-format)
- [Configuration Files](#configuration-files)
- [Commands](#commands)
- [Related Projects](#-related-projects)

## Principles

Nebula is built around the following core principles:

**Template-driven builds**: Use reusable templates instead of project-specific TCL scripts. Write a template once, use it across multiple projects with different configurations.

**Explicit file control**: No more guessing which files got included. Filelists give you precise control over RTL sources, IP cores, and constraints.

**Clean separation**: Source files stay in their original locations. Build outputs go to separate directories. No copying or polluting your source tree.

**Reproducible builds**: Same inputs always produce the same Vivado project. Lock down exact file sets for releases and silicon bring-up.

**Zero dependencies**: Single compiled Rust binary. No Python interpreter versions, no package conflicts, no environment setup.

## Installation

To use Nebula, download the precompiled binary for your platform from the [releases page](https://github.com/saturn77/nebula/releases), or build from source:

```bash
git clone https://github.com/saturn77/nebula
cd nebula
cargo install --path .
```

If you have Rust installed, you can also install directly from source:
```bash
cargo install --git https://github.com/saturn77/nebula
```

## Quick Start

### 1. Generate a simple project
```bash
nebula generate \
  --template templates/vecheck.tcl \
  --proj-name my_test_project \
  --target-dir ./builds \
  --origin-dir /path/to/your/rtl \
  --fpga-part xc7s25csga225-1 \
  --filelist vecheck_filelist.f
```

### 2. Generate and run Vivado 
```bash
nebula generate \
  --template templates/basic.tcl \
  --proj-name my_fpga_design \
  --target-dir ~/fpga_builds \
  --origin-dir . \
  --fpga-part xc7a35tcsg324-1 \
  --filelist basic_filelist.f \
  --compile \
  --run-vivado
```

### 3. Self-contained example (no external dependencies!)
This self contained examples uses the files from the [VeCheck](https://github.com/saturn77/VeCheck) repository.

```bash
# Uses the included VeCheck SystemVerilog files - RON format
nebula generate \
  --template templates/vecheck.tcl \
  --config examples/vecheck_example.ron \
  --run-vivado --vivado-gui

# Also supports JSON format  
nebula generate \
  --template templates/vecheck.tcl \
  --config examples/vecheck_example.json \
  --run-vivado --vivado-gui
```

## 📁 Project Structure

```
your-fpga-project/
├── hdl/                    # RTL source files
│   ├── top_level.sv
│   ├── packages.sv
│   └── modules/
├── ip/                     # IP cores and block designs
│   ├── clk_wiz_0/
│   └── my_block_design.tcl
├── constraints/            # Timing and pin constraints
│   ├── pins.xdc
│   └── timing.xdc
└── builds/                 # Generated Vivado projects (separate)
    └── my_project/

# Nebula includes a complete working example:
nebula/
└── examples/
    └── vecheck/            # Self-contained SystemVerilog test
        ├── hdl/
        │   ├── top_level.sv
        │   ├── CounterPkg.sv
        │   └── *.sv
        └── README.md
```

## 🔧 Templates

Nebula uses [Tera](https://tera.netlify.app/) templating (similar to Jinja2) for flexible TCL generation.

### Available Templates

- **`basic.tcl`** - Generic FPGA project template (Arty A7, etc.)
- **`vecheck.tcl`** - Simple SystemVerilog test projects (Cmod S7)

### Template Variables

All templates support these variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `proj_name` | Project name | required |
| `target_dir` | Build directory | required |
| `origin_dir` | Source files directory | required |
| `fpga_part` | FPGA part number | varies by template |
| `board_part` | Board part (optional) | none |
| `top_module` | Top module name | "top_level" |
| `date_time` | Timestamp | current time |
| `compile` | Run synthesis/implementation | false |
| `filelist_path` | RTL filelist path | auto-detected |
| `ip_filelist` | IP cores filelist path | auto-detected |
| `constraints_filelist` | Constraints filelist path | auto-detected |

## 📝 Filelists

Nebula uses separate filelists for different file types that includes
1. RTL source files (`*_filelist.f`)
2. IP cores and block designs (`*_ip.f`)
3. Constraints files (`*_constraints.f`)
   
Filelists are simple text files with one file path per line, relative to the `origin_dir`.

### RTL Filelist (`*_filelist.f`)
```bash
# SystemVerilog packages first
top/SystemPkg.sv
top/types.sv

# Main modules  
top/top_level.sv
peripherals/uart.sv
timers/counter.sv
```

### IP Filelist (`*_ip.f`)
```bash
# IP cores (.xci files)
clk_wiz_0/clk_wiz_0.xci
vio_debug/vio_debug.xci

# Block designs (.tcl scripts)
microblaze_system/mb_system.tcl
```

### Constraints Filelist (`*_constraints.f`)
```bash
# Pin assignments
pins_zcu104.xdc

# Timing constraints
timing_serdes.xdc
```

## 🛠️ Command Reference

### Generate Command
```bash
nebula generate [OPTIONS]
```

**Core Options:**
- `--template <PATH>` - TCL template file
- `--proj-name <NAME>` - Project name
- `--target-dir <DIR>` - Build directory
- `--origin-dir <DIR>` - Source files directory
- `--fpga-part <PART>` - FPGA part number

**Filelist Options:**
- `--filelist <FILE>` - RTL filelist (relative to template dir)
- `--config <FILE>` - JSON/RON config file

**Vivado Integration:**
- `--run-vivado` - Execute TCL script in Vivado batch mode
- `--vivado-gui` - Open Vivado GUI after batch processing
- `--compile` - Run synthesis and implementation
- `--dry-run` - Show what would be executed

**Other Options:**
- `--with-timestamp` - Add timestamp to generated files
- `--output <FILE>` - Output TCL file path

### Other Commands
```bash
nebula list --dir templates/     # List available templates
nebula example --example-type basic --format json  # Show example configs
```

## 📋 Examples

### Example 1: VeCheck SystemVerilog Test
```json
{
  "proj_name": "sv_language_test",
  "target_dir": "./builds",
  "origin_dir": "/path/to/VeCheck",
  "fpga_part": "xc7s25csga225-1",
  "board_part": "digilentinc.com:cmod_s7-25:part0:1.0",
  "top_module": "top_level",
  "compile": true
}
```

### Example 2: Basic FPGA Project
```json
{
  "proj_name": "arty_demo_project", 
  "target_dir": "./builds",
  "origin_dir": "/path/to/my_project",
  "fpga_part": "xc7a35tcsg324-1",
  "board_part": "digilentinc.com:arty-a7-35:part0:1.0",
  "compile": true,
  "synth_strategy": "Flow_AreaOptimized_high",
  "synth_jobs": 4
}
```

## 🔄 Typical Workflow

1. **Organize your project** with separate `hdl/`, `ip/`, and `constraints/` directories
2. **Create filelists** specifying exactly which files to include  
3. **Choose a template** that matches your project type
4. **Generate and test** with `--dry-run` first
5. **Build the project** with `--run-vivado --compile`
6. **Iterate** by updating filelists and regenerating

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)  
5. Open a Pull Request

### Adding New Templates

Templates are welcomed! Good templates should:
- Support common FPGA parts/boards for their domain
- Use separate filelists for RTL/IP/constraints
- Include helpful comments and error handling
- Have sensible defaults for template variables

## 📄 License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## 🌟 Related Projects

- [Bender](https://github.com/pulp-platform/bender) - Dependency management tool for SystemVerilog and VHDL
- [FuseSoC](https://github.com/olofk/fusesoc) - Package manager and build abstraction tool for FPGA/ASIC development
- [Edalize](https://github.com/olofk/edalize) - Python library for interfacing EDA tools
- [hdlmake](https://ohwr.org/project/hdl-make) - Tool for generating HDL makefiles

---

*Hardware is Software, and Software is Hardware.* Make your life easier with Nebula!