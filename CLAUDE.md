# CLAUDE.md - Development Plan & Architecture Notes

**Project:** TrajecSimuGUI - Rocket Trajectory Simulation Platform  
**Last Updated:** 2026-04-15  
**Status:** Phase 1 - Core Engine Architecture (In Progress)

## Project Overview

Rust-based monorepo combining JSBSim integration with custom simulation engines.

### Structure
```
crates/
  ├── simulator_core/    # Physics engine (PRIORITY: HIGHEST)
  ├── simulator_cli/     # CLI interface (PRIORITY: LOW)
  └── simulator_gui/     # Tauri GUI (PRIORITY: MEDIUM)
```

## Setup Complete ✅

### Completed
- [x] Rust workspace configuration
- [x] Three-crate monorepo structure
- [x] Core library scaffolding
  - SimulationParams (input parameters with validation)
  - SimulationOutput (trajectory data structures)
  - Simulator trait (pluggable backend abstraction)
- [x] Custom ballistic simulator (basic physics)
- [x] JSBSim wrapper skeleton
- [x] Error handling framework
- [x] CLI argument parsing
- [x] Tauri GUI backend stubs
- [x] Comprehensive README roadmap
- [x] Tauri configuration

## Implementation Roadmap

### Phase 1: Core Simulator Engine 🔴 (CURRENT)

#### 1.1 Parameter System ✅
- Define `SimulationParams` struct
- Implement validation
- [ ] Add preset system (Estes, Aerotech, etc.)
- [ ] JSON config file loading

#### 1.2 Output Data Structure ✅
- `SimulationOutput` and state types
- [ ] CSV export format
- [ ] GeoJSON trajectory export

#### 1.3 Simulator Trait ✅
- Abstract `Simulator` trait
- Plugin architecture ready
- [ ] Factory pattern for simulator selection

#### 1.4 Custom Ballistic Simulator ✅
- Basic gravity + velocity integration
- [ ] **NEXT:** Atmospheric density model (ISO 2533)
- [ ] Drag coefficient effects (Cd modeling)
- [ ] Thrust/burn profile
- [ ] Stability analysis (angle of attack tracking)

#### 1.5 JSBSim Wrapper ⏳
- [ ] PyO3 bindings (Rust ↔ JSBSim Python)
- [ ] Parameter mapping conversion
- [ ] State extraction
- [ ] Aircraft XML loading
- [ ] Integration testing

#### 1.6 Data Analysis Module ⏳
- [ ] Apogee/landing detection
- [ ] Flight statistics
- [ ] Event timeline (burnout, apogee, landing)
- [ ] Stability metrics

### Phase 2: GUI Application 🟡 (After 1.4)
- Parameter input form
- 2D/3D visualization
- Telemetry display
- SvelteKit integration

### Phase 3: CLI Tool 🟢 (After 2)
- Batch execution
- Parameter optimization
- Export utilities

## Next Steps (Priority Order)

1. **Enhance custom simulator physics**
   - Add atmospheric density (ISO 2533 standard atmosphere)
   - Implement drag coefficient effects
   - Add thrust/burn curve modeling
   - Test with real rocket data

2. **JSBSim integration**
   - Set up PyO3 for Python-Rust binding
   - Map SimulationParams → JSBSim properties
   - Extract output from JSBSim state
   - Create adapter layer

3. **Data analysis**
   - Statistics module (apogee, range, duration)
   - Event detection
   - Stability analysis

4. **GUI development**
   - Convert SvelteKit to use simulator_core
   - Build parameter input form
   - Implement 2D trajectory visualization
   - Add telemetry display

## Build Commands

```bash
# Build all
cargo build --release

# Build individual crates
cargo build -p simulator_core
cargo build -p simulator_cli
cargo build -p simulator_gui

# Test
cargo test -p simulator_core

# Run CLI
cargo run -p simulator_cli -- --altitude 1000 --velocity 150

# Run GUI (dev)
cd crates/simulator_gui && cargo tauri dev

# Formatting
cargo fmt && cargo clippy --all-targets
```

## Architecture Notes

### Trait-Based Design
All simulators implement `Simulator` trait:
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
SimulationParams → Validation → Simulator.initialize() → Step loop → SimulationOutput

### Error Handling
Custom `SimulatorError` enum with specific error types for each subsystem.

## Integration Notes

- **JSBSim Submodule:** Already present at `jsbsim/`
- **SvelteKit Frontend:** Exists in `src/`, needs integration with simulator_core
- **Node.js Config:** Preserved in `package.json` for frontend build
- **Original Tauri Config:** Replaced with monorepo structure

## Known Limitations

- Custom simulator uses simple ballistic model (no aerodynamics yet)
- JSBSim integration not implemented (Python binding pending)
- No atmospheric effects except gravity
- No thrust modeling
- GUI not integrated with simulator_core

## Key Files to Know

- [Cargo.toml](Cargo.toml) - Workspace config
- [simulator_core/src/lib.rs](crates/simulator_core/src/lib.rs) - Core module
- [simulator_core/src/parameters.rs](crates/simulator_core/src/parameters.rs) - Input params
- [simulator_core/src/output.rs](crates/simulator_core/src/output.rs) - Output structures
- [simulator_core/src/custom_simulator.rs](crates/simulator_core/src/custom_simulator.rs) - Physics engine
- [README.md](README.md) - Full documentation and roadmap

## Development Tips

1. Use trait bounds for generic simulator types
2. Keep SimulationParams serializable for IPC
3. Add tests in each module's `mod tests` section
4. Use `cargo clippy` to catch common mistakes
5. Document public APIs with doc comments

## Questions & Notes

- JSBSim Python API needs PyO3 wrapper - plan this carefully
- Atmospheric model should follow ISO 2533 standard
- Consider storing trajectory history in SimulatorState for debugging
- GUI visualization will need efficient trajectory data structures

**Development (frontend only):**
```
pnpm dev
```

**Development (full Tauri app):**
```
pnpm tauri dev
```

**Build frontend:**
```
pnpm build
```

**Build Tauri app:**
```
pnpm tauri build
```

**Type-check Svelte:**
```
pnpm check
```

**Package manager:** pnpm (not npm or yarn)

## Architecture

This is a **Tauri v2** desktop app using **SvelteKit** (SPA mode) + **TypeScript** on the frontend and **Rust** on the backend.

### Key constraints
- SvelteKit runs as a static SPA (no SSR) — `ssr = false` in `src/routes/+layout.ts`, adapter-static with `fallback: "index.html"`.
- Tauri's dev server expects the Vite frontend at `http://localhost:1420`.
- Frontend is built to `../build` (relative to `src-tauri/`).

### Frontend (`src/`)
- **Svelte 5** with runes (`$state`, etc.)
- **Tailwind CSS v4** (via `@tailwindcss/vite` plugin — no `tailwind.config.js` needed)
- **MapLibre GL** for map rendering with local raster tiles served from `public/tiles/{z}/{x}/{y}.jpg`
- Map is centered on the Fukuoka area (130.4, 33.6) and uses locally hosted tiles at zoom level 10
- `src/lib/components/Map.svelte` — MapLibre map component; loads local aerial tiles, renders GeoJSON route lines

### Backend (`src-tauri/`)
- Rust entry: `src-tauri/src/lib.rs` (logic) + `src-tauri/src/main.rs` (binary entry)
- Tauri commands are registered in `lib.rs` via `invoke_handler` and called from the frontend with `invoke()` from `@tauri-apps/api/core`
- Currently has a sample `greet` command

### Adding Tauri commands
1. Define `#[tauri::command]` fn in `src-tauri/src/lib.rs`
2. Register it in `tauri::generate_handler![...]`
3. Call from frontend: `invoke("command_name", { args })` (returns a Promise)
