# TrajecSimuGUI - Rocket Trajectory Simulation Platform

A Rust-based monorepo for rocket trajectory simulation combining JSBSim integration with custom simulation engines,
providing CLI and Tauri GUI interfaces.
❯ perf stat -ddd cargo run --profile release -p simulator_cli -- landing-area --config
crates/simulator_cli/examples/noshiri_2025/config.yaml --out-dir crates/simulator_cli/examples/noshiri_2025/out

# License

This project is dual-licensed under either the MIT license or the Apache License, Version 2.0, at your option.

## Project Overview

TrajecSimuGUI is a comprehensive rocket simulation platform designed to:

- Wrap and integrate the JSBSim flight dynamics library for accurate aerodynamic modeling
- Provide custom lightweight simulators for quick trajectory calculations
- Accept rocket-specific parameters and convert them for various simulation engines
- Parse and analyze simulation output data
- Deliver results through CLI and interactive Tauri GUI applications

## Project Structure

This is a Cargo workspace with three main crates:

```
TrajecSimuGUI/
├── Cargo.toml                    # Workspace configuration
├── README.md                     # This file
│
├── crates/
│   ├── simulator_core/           # Core simulation engine (PRIORITY: HIGHEST)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Main module with Simulator trait
│   │       ├── error.rs          # Error types
│   │       ├── parameters.rs     # Input parameter structures
│   │       ├── output.rs         # Output data structures
│   │       ├── jsbsim_wrapper.rs # JSBSim integration
│   │       └── custom_simulator.rs # Custom ballistic simulator
│   │
│   ├── simulator_cli/            # CLI interface (PRIORITY: LOW)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs           # CLI implementation
│   │
│   └── simulator_gui/            # Tauri GUI application (PRIORITY: MEDIUM)
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── build.rs
│       └── src/
│           └── main.rs           # Tauri backend commands
│
├── jsbsim/                       # JSBSim submodule
├── src/                          # SvelteKit frontend
├── src-tauri/                    # Original Tauri config (deprecated)
└── package.json                  # Node.js/SvelteKit config
```

## Implementation Roadmap

### Phase 1: Core Simulator Engine (🔴 Current Priority)

**Status:** Architecture in progress  
**Target:** Complete by iteration 2

#### 1.1 Parameter System

- [] Define `SimulationParams` struct for rocket inputs
  - Altitude, velocity, pitch/roll/yaw angles
  - Mass, position (lat/lon)
  - Simulation duration and time step
- [] Implement parameter validation
- [ ] Create parameter preset system (common rocket types)
- [ ] JSON serialization support

#### 1.2 Output Data Structure

- [] Define `SimulationOutput` and related data types
  - `SimulationState`: position, velocity, attitude, acceleration
  - Trajectory metrics (downrange, max altitude)
- [] Implement JSON serialization
- [ ] Create trajectory data export formats (CSV, GeoJSON)

#### 1.3 Simulator Trait & Interface

- [] Define `Simulator` trait for pluggable backends
  - `initialize()`, `step()`, `reset()`
  - `get_output()`, `is_complete()`
- [] Create abstract interface for parameter conversion

#### 1.4 Custom Ballistic Simulator

- [] Implement simple ballistic trajectory model
  - Basic physics: gravity, velocity integration
  - Position tracking and altitude computation
- [ ] Add atmospheric density model
- [ ] Add drag coefficient effects
- [ ] Add thrust/burn modeling

#### 1.5 JSBSim Wrapper

- [] Skeleton structure for JSBSim integration
- [ ] Python binding setup (JSBSim → Rust via PyO3)
- [ ] Parameter mapping: SimulationParams → JSBSim properties
- [ ] Output extraction from JSBSim state
- [ ] Aerodynamic model loading (aircraft XML configs)

#### 1.6 Data Analysis Module

- [ ] Trajectory statistics (apogee, landing distance, flight time)
- [ ] Stability analysis (angle of attack tracking)
- [ ] Event detection (burnout, apogee, landing)

### Phase 2: GUI Application (🟡 Medium Priority)

**Status:** Not started  
**Target:** Begin after Phase 1.4 completion

#### 2.1 UI Layout

- [ ] Parameter input form
- [ ] Real-time 2D/3D trajectory visualization
- [ ] Flight data telemetry display
- [ ] Results export interface

#### 2.2 Tauri Backend Integration

- [x] Define IPC commands (`run_simulation`, `get_status`)
- [ ] Implement async simulation runner
- [ ] Add result caching/history

#### 2.3 SvelteKit Frontend

- [ ] Convert existing SvelteKit project to use simulator_core
- [ ] Implement visualization with Maplibre-gl
- [ ] Add parameter persistence (localStorage)

#### 2.4 Advanced Features

- [ ] Real-time simulation progress
- [ ] Comparison of multiple simulations
- [ ] Rocket library management

### Phase 3: CLI Application (🟢 Low Priority)

**Status:** Skeleton created  
**Target:** Begin after Phase 2

#### 3.1 Basic CLI Commands

- [x] Argument parsing with clap
- [ ] `run` - Execute simulation from parameters
- [ ] `batch` - Run multiple simulations
- [ ] `export` - Export results to various formats
- [ ] `validate` - Check parameter files

#### 3.2 Advanced Features

- [ ] Optimization mode (parameter sweep)
- [ ] Comparison utilities

## Getting Started

### Prerequisites

- Rust 1.70+ (`rustup`)
- Node.js 18+ (for Tauri frontend)
- Python 3.8+ (for JSBSim)
- macOS, Linux, or Windows with development tools

### Installation

```bash
# Clone repository with submodules
git clone --recursive https://github.com/misohiyoko/TrajecSimuGUI.git
cd TrajecSimuGUI

# Install Rust dependencies
cargo build

# Install Node dependencies
npm install

# (Optional) Set up JSBSim Python bindings
cd jsbsim && python setup.py develop
```

### Building

#### Build all crates

```bash
cargo build --release
```

#### Build specific crates

```bash
# Core library
cargo build --release -p simulator_core

# CLI tool
cargo build --release -p simulator_cli

# GUI application
cargo build --release -p simulator_gui
```

### Running

#### Test custom simulator

```bash
cargo test -p simulator_core
```

#### Run CLI

```bash
cargo run --release -p simulator_cli -- \
  --altitude 100 \
  --velocity 150 \
  --angle 45 \
  --output results.json
```

#### Run GUI (development)

```bash
cd crates/simulator_gui
cargo tauri dev
```

#### Run GUI (production build)

```bash
cd crates/simulator_gui
cargo tauri build
```

## Architecture Details

### Simulator Trait Pattern

All simulators implement the `Simulator` trait, allowing:

- Pluggable backends (JSBSim, custom models, future simulators)
- Unified parameter and output interfaces
- Easy testing and comparison

```rust
pub trait Simulator: Send + Sync {
    fn initialize(&mut self, params: &SimulationParams) -> Result<()>;
    fn step(&mut self, dt: f64) -> Result<()>;
    fn reset(&mut self) -> Result<()>;
    fn get_output(&self) -> Result<SimulationOutput>;
    fn is_complete(&self) -> bool;
}
```

### Data Flow

1. **Input:** User provides `SimulationParams`
2. **Validation:** Parameters checked for consistency
3. **Conversion:** Parameters adapted for target simulator
4. **Execution:** Simulator runs step-by-step
5. **Collection:** Output collected at each step
6. **Analysis:** Trajectory statistics computed
7. **Output:** Results exported (JSON, CSV, visualization)

## Current Implementation Status

✅ = Completed  
🔄 = In Progress  
⏳ = Planned

| Component           | Status | Notes                                 |
| ------------------- | ------ | ------------------------------------- |
| Workspace structure | ✅     | Cargo.toml configured                 |
| Core library stub   | ✅     | Module organization done              |
| Parameter system    | ✅     | Basic validation implemented          |
| Output structures   | ✅     | All data types defined                |
| Simulator trait     | ✅     | Abstraction defined                   |
| Custom simulator    | ✅     | Basic ballistic model working         |
| JSBSim wrapper      | ⏳     | Skeleton only, Python binding pending |
| Data analysis       | ⏳     | Statistics module not started         |
| CLI basic           | ✅     | Argument parsing done                 |
| CLI advanced        | ⏳     | Batch and export pending              |
| Tauri backend       | ⏳     | Basic IPC commands defined            |
| SvelteKit frontend  | ⏳     | Not integrated with simulator yet     |
| 2D visualization    | ⏳     | Not started                           |
| 3D visualization    | ⏳     | Not started                           |

## Development Workflow

1. **Feature branches**: Create branches from `main` for features/fixes
2. **Testing**: Write tests in each crate's `mod tests` section
3. **Documentation**: Update this README with progress
4. **Code style**: Use `cargo fmt` and `cargo clippy`

### Useful Commands

```bash
# Format code
cargo fmt

# Lint code
cargo clippy --all-targets

# Run tests
cargo test

# Generate documentation
cargo doc --open

# Check for unused dependencies
cargo udeps
```

## Dependencies

### simulator_core

- `serde`, `serde_json`: Data serialization
- `nalgebra`: Linear algebra for physics
- `ndarray`: Numerical computing
- `tokio`: Async runtime
- `log`, `env_logger`: Logging

### simulator_gui

- `tauri`: Desktop app framework
- `serde`, `serde_json`: IPC serialization

### simulator_cli

- `clap`: Command-line parsing
- `csv`: CSV export

## Known Issues & Limitations

- JSBSim Python binding integration not yet implemented
- Custom simulator is simple ballistic model without aerodynamics
- Atmospheric density modeling not included
- No drag modeling in custom simulator
- No thrust/engine modeling yet
- GUI visualization not implemented

## Future Enhancements

- [ ] Motor impulse profiles and staging
- [ ] Aerodynamic coefficient database
- [ ] Apogee detection and ejection charge timing
- [ ] Recovery system simulation
- [ ] Wind and atmospheric effects
- [ ] Multi-stage rocket support
- [ ] Real-time telemetry stream support
- [ ] Data logging to SD card simulation

## Contributing

See CONTRIBUTING.md (to be created)

## License

MIT License - See LICENSE file

## References

- [JSBSim Documentation](https://jsbsim.sourceforge.net/)
- [Tauri Documentation](https://tauri.app/)
- [Rust Book](https://doc.rust-lang.org/book/)
- [SvelteKit Documentation](https://kit.svelte.dev/)

## Contact

For questions or suggestions, open an issue on GitHub.

---

**Last Updated:** 2026-04-15  
**Maintainer:** misohiyoko  
**Project Status:** Early Development (Pre-alpha)
