# Nebula FPGA Build Tool - Just Recipes
# Run `just --list` to see all available commands

# Default recipe - show available commands
default:
    @just --list

# Build and install nebula from source
install:
    cargo install --path .

# Build nebula in development mode
build:
    cargo build

# Run tests
test:
    cargo test

# Check code formatting and lints
check:
    cargo fmt --check
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Clean build artifacts
clean:
    cargo clean
    rm -rf builds/
    rm -rf examples/vecheck/builds/

# Run the VeCheck SystemVerilog example (RON config)
vecheck-ron:
    cargo run -- generate \
      --template templates/vecheck.tcl \
      --config examples/vecheck_example.ron

# Run the VeCheck SystemVerilog example (JSON config)  
vecheck-json:
    cargo run -- generate \
      --template templates/vecheck.tcl \
      --config examples/vecheck_example.json

# Run VeCheck example with Vivado GUI
vecheck-gui:
    cargo run -- generate \
      --template templates/vecheck.tcl \
      --config examples/vecheck_example.ron \
      --run-vivado --vivado-gui

# Run VeCheck example with synthesis and implementation
vecheck-build:
    cargo run -- generate \
      --template templates/vecheck.tcl \
      --config examples/vecheck_example.ron \
      --compile --run-vivado

# Generate basic FPGA project for Arty A7-35
arty PROJECT_NAME ORIGIN_DIR:
    cargo run -- generate \
      --template templates/basic.tcl \
      --proj-name {{PROJECT_NAME}} \
      --target-dir ./builds \
      --origin-dir {{ORIGIN_DIR}} \
      --fpga-part xc7a35tcsg324-1 \
      --board-part "digilentinc.com:arty-a7-35:part0:1.0" \
      --filelist basic_filelist.f

# Generate basic project with compilation
arty-build PROJECT_NAME ORIGIN_DIR:
    cargo run -- generate \
      --template templates/basic.tcl \
      --proj-name {{PROJECT_NAME}} \
      --target-dir ./builds \
      --origin-dir {{ORIGIN_DIR}} \
      --fpga-part xc7a35tcsg324-1 \
      --board-part "digilentinc.com:arty-a7-35:part0:1.0" \
      --filelist basic_filelist.f \
      --compile --run-vivado

# Generate Cmod S7 project
cmod PROJECT_NAME ORIGIN_DIR:
    cargo run -- generate \
      --template templates/vecheck.tcl \
      --proj-name {{PROJECT_NAME}} \
      --target-dir ./builds \
      --origin-dir {{ORIGIN_DIR}} \
      --fpga-part xc7s25csga225-1 \
      --board-part "digilentinc.com:cmod_s7-25:part0:1.0" \
      --filelist vecheck_filelist.f

# Dry run - show what would be generated without running
dry-run TEMPLATE CONFIG:
    cargo run -- generate \
      --template {{TEMPLATE}} \
      --config {{CONFIG}} \
      --dry-run

# List available templates
templates:
    cargo run -- list --dir templates/

# Show example configurations
examples:
    @echo "Available example configurations:"
    @echo "  examples/vecheck_example.ron  - VeCheck SystemVerilog test (RON format)"
    @echo "  examples/vecheck_example.json - VeCheck SystemVerilog test (JSON format)"
    @echo "  examples/basic_example.ron    - Basic FPGA project template"

# Create a new project structure
new-project PROJECT_NAME:
    #!/usr/bin/env bash
    mkdir -p {{PROJECT_NAME}}/{hdl,ip,constraints}
    mkdir -p {{PROJECT_NAME}}/hdl/{top,peripherals,packages}
    echo "# RTL source files" > {{PROJECT_NAME}}/{{PROJECT_NAME}}_filelist.f
    echo "hdl/packages/SystemPkg.sv" >> {{PROJECT_NAME}}/{{PROJECT_NAME}}_filelist.f  
    echo "hdl/top/top_level.sv" >> {{PROJECT_NAME}}/{{PROJECT_NAME}}_filelist.f
    echo "# IP cores" > {{PROJECT_NAME}}/{{PROJECT_NAME}}_ip.f
    echo "# Constraints" > {{PROJECT_NAME}}/{{PROJECT_NAME}}_constraints.f
    echo "Created project structure in {{PROJECT_NAME}}/"

# Package for release (requires cross compilation setup)
package:
    cargo build --release
    strip target/release/nebula
    @echo "Release binary: target/release/nebula"

# Development workflow - format, check, test, build  
dev:
    just fmt
    just check  
    just test
    just build