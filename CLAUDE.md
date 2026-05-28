# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Run the full Tauri desktop app (dev mode — starts Vite on :1420 then opens the window)
pnpm tauri dev

# Build the distributable app bundle
pnpm tauri build

# Frontend only (SvelteKit/Vite)
pnpm dev           # dev server on http://localhost:1420
pnpm build         # static output → build/
pnpm run check     # svelte-check + tsc type checking
pnpm run fix       # ESLint + Prettier + cargo fmt + cargo clippy (all at once)

# Rust only
cargo build -p simulator_core
cargo build -p simulator_cli
cargo build --workspace

# Test all (74 tests)
cargo test --workspace

# Test a single crate
cargo test -p simulator_core
cargo test -p simulator_cli

# Test a single module or function
cargo test -p simulator_core -- launch_rail
cargo test -p simulator_core -- parachute::tests::transient_converges

# Lint / format
cargo fmt
cargo clippy --all-targets

# Run CLI
cargo run -p simulator_cli -- run -c crates/simulator_cli/examples/minimal/config.yaml --out-dir out
cargo run -p simulator_cli -- validate -c path/to/config.yaml
cargo run -p simulator_cli -- inspect  -c path/to/config.yaml   # pretty-print assembled params
cargo run -p simulator_cli -- landing-area -c path/to/config.yaml  # parallel wind sweep

# Snapshot tests (insta)
cargo insta review   # after a snapshot value legitimately changes
```

## Repository Structure

```
crates/
  simulator_core/   # physics engine library (no I/O)
  simulator_cli/    # CLI binary — config loading, file output, DEM
  tile_cache/       # aerial imagery and DEM tile caching (GSI Japan)
src/                # SvelteKit frontend (Tauri GUI)
  routes/           # +page.svelte (main app), +layout.ts (SPA mode, ssr=false)
  lib/
    components/     # Svelte UI components
    types/          # TypeScript interfaces mirroring Rust config structs
    utils/          # tileBaseUrl.ts, configMerge.ts, updater.ts, path.ts
src-tauri/          # Tauri v2 backend (Rust)
  src/lib.rs        # invoke handlers, tile:// URI scheme, app setup
```

## simulator_core Architecture

### Phase pipeline

The `SimulationOrchestrator` (orchestrator.rs) drives three sequential phases:

```
OnRail   → LaunchRailStage   (simple_simulator/launch_rail.rs)
           1-DOF kinematics along the rail axis, with ISA atmosphere and Cd0 drag
Ballistic → JsbSimStage      (simple_simulator/jsbsim_stage.rs)
           wraps JsbSimSimulator (full 6-DOF via JSBSim C++ binding)
           detects EngineBurnout (from thrust_table), Apogee, Landed
Parachute → ParachuteStage   (simple_simulator/parachute.rs)
           3-D point mass with terminal-velocity drag model
           terminal velocity table values are at standard sea-level density;
           corrected at altitude via v_term_actual = v_term_SL * sqrt(ρ₀/ρ)
```

Each phase implements `StageRunner { initialize, step }` and returns `StageStepOutput { state, events, completed }`.

### Key data structures

**`SimulationState`** (`output.rs`) — per-step snapshot shared by all phases. All values in SI. Coordinates:

- `position.alt_agl_m` stores **MSL altitude** (not true AGL); named for historical reasons
- `position.local_x_m/y_m` — distance in launch-yaw and perpendicular directions
- `velocity.u/v/w_mps` — body-axis forward / lateral / down components

**`Trajectory`** (`output.rs`) — Structure of Arrays (SoA): 31 parallel `Vec<f64>` columns. Use `push(&state)`, `get_state(i)`, `last_state()`, and `row_iter()` (yields reconstructed `SimulationState`s).

**`UnifiedSimulationOutput`** — top-level result: `mainline: SimulationOutput`, `parachute_branch: SimulationOutput`, `events: Vec<EventStamp>`, `analysis: AnalysisOutput`.

**`EventStamp`** — carries `EventKind`, `sim_time_sec`, `EventSource`, and `Option<SimulationState>` snapshot. Events are sparse (~15 per run) and use AoS layout.

### Post-simulation analysis (`analysis.rs`)

`analysis::analyze(&mut output, &params)` detects peak events over the mainline trajectory (MaxQ, MaxAxialAcceleration, MaxLateralAcceleration, MaxAngularRate, MaxThrust, MaxAirspeed, MaxDynamicPressureAlpha) and appends them as derived `EventStamp`s, then time-sorts all events.

### Coordinate conventions

- **Wind table**: `[altitude_m, speed_mps, direction_deg]` — meteorological "from" convention (dir 0 = from north). ENU north component = `-speed * cos(dir)`.
- **Attitude**: pitch 90° = vertical launch.
- **Altitude**: `launch_env.elevation` is MSL elevation of the pad. `position.alt_agl_m` = `elevation + height_above_pad` throughout the codebase.

### Standard atmosphere (`standard_atmosphere.rs`)

ISA 1976. `sample_atmosphere(alt_msl_m) -> AtmosphereSample { temperature_k, pressure_pa, density_kg_m3, sound_speed }`. Clamped at 86 km.

## simulator_cli Architecture

### Config format

User-facing YAML (`config.rs` + `assemble.rs`). Tables (thrust, aero, cp, terminal velocity) are referenced as CSV paths relative to the config file.

```yaml
launch:
  {
    latitude,
    longitude,
    elevation,
    rail_length,
    pitch,
    yaw,
    wind_speed_mps,
    wind_direction_deg,
    ...,
  }
body: { diameter, dry_mass_with_fuel_section, cg, inertia }
engine:
  {
    thrust_table,
    thruster_pos,
    tank: { position, tank_contents },
    fuel: { ... },
  }
aero:
  { cp_at_launch, cp_mach_table, cd0_alpha_mach_table, cn_table, cs_table, ... }
parachute: { terminal_velocity_table, deploy_delay_sec } # optional
sim: { flight_duration, time_step, csv_sample_interval, kml_sample_interval }
```

See `crates/simulator_cli/examples/minimal/config.yaml` for a complete reference.

### `Cd0AlphaMachTable`

Bilinear interpolation table. `mach_keys` must have ≥ 2 entries; each row is `[alpha_key, cd_at_mach0, cd_at_mach1, ...]`. Test fixtures must use at least a 2×2 table.

### Output files (written by `runner.rs`)

| File                             | Content                                                       |
| -------------------------------- | ------------------------------------------------------------- |
| `mainline.csv` / `parachute.csv` | Trajectory rows decimated by `csv_sample_interval`            |
| `events.csv` / `events.json`     | All events with full state payload                            |
| `summary.json`                   | Apogee, max speed, flight time, landing point                 |
| `trajectory.kml`                 | LineStrings + event placemarks, `altitudeMode=absolute` (MSL) |

### DEM elevation refinement (`dem.rs`, `refine_landing.rs`)

GSI Japan tiles (zoom 15, `cyberjapandata.gsi.go.jp`). Cached under `{cache_dir}/trajec_simu_dem/15/`. `refine_one()` walks each trajectory backwards to find the actual terrain crossing and overwrites the `Landed`/`ParachuteLanded` event state.

True AGL = `position.alt_agl_m - dem_elevation_at_latlon`.

### Landing area sweep (`landing_area.rs`)

Rayon-parallel sweep over wind speed × direction. Power-law wind profile applied. Each condition runs the full simulate + refine + write pipeline.

## JSBSim Backend

`JsbSimSimulator` wraps JSBSim via a C++ cxx binding (`jsbsim/ffi.rs`). The smoke test (`tests/jsbsim_smoke.rs`) is `#[ignore]` — run manually when JSBSim is available. The JSBSim aircraft XML is generated from `RocketParams` by `xml_gen/`.

## Frontend Architecture

**Stack**: SvelteKit 2 + Svelte 5 + Vite 6 + Tailwind CSS 4. Compiled to a static site (`adapter-static`, `fallback: "index.html"`) — no SSR.

### State management

All app state lives in `src/routes/+page.svelte` as Svelte 5 `$state()` runes. There are no separate stores. Key state buckets:
- `config: AppConfig` — mirrors the YAML config struct (`src/lib/types/config.ts`)
- `result / landingAreaResult` — simulation outputs returned by Tauri invoke
- `activeTab`, `running`, `progressMsg`, `mapLoaded` — UI coordination
- Display toggles: `showBallisticCourse`, `showParachuteCourse`, etc. (passed as props to map components)

Persistent settings (output dir, DEM flag, spreadsheet URL) are saved via `@tauri-apps/plugin-store` to `app-settings.json`.

### Map components

`SummaryMap.svelte` and `ChartMap.svelte` both use MapLibre GL JS + deck.gl (`MapboxOverlay`). Tile URLs are built with `getTileBaseUrl()` (`src/lib/utils/tileBaseUrl.ts`):
- macOS/Linux: `tile://localhost` (native Tauri custom scheme)
- Windows: `https://tile.localhost` (WebView2 remaps `tile://` → `https://tile.localhost/` so Fetch API in web workers can load it)

### Svelte 5 rune conventions

Components declare props with `interface Props` + destructured `$props()`. Reactive side-effects use `$effect()`. Bindable outputs (e.g., `mapLoaded`) use `$bindable()`.

## Tauri Backend (`src-tauri/src/lib.rs`)

### Invoke commands

| Command | Description |
|---|---|
| `load_config(path)` | Parse YAML → `AppConfig` |
| `save_config(config, savePath)` | Serialize `AppConfig` → YAML |
| `validate_config(config)` | Return validation errors |
| `run_simulation(config, outDir, noDem)` | Full sim run; emits `"sim-progress"` events |
| `run_landing_area(config, outDir, noDem, directions, speedMax, speedSteps)` | Parallel wind sweep |
| `load_kml_file(path)` | Read KML/KMZ → string |
| `fetch_google_sheet(url)` | Pull config from Google Sheets (requires auth) |

### Custom tile protocol

`register_asynchronous_uri_scheme_protocol("tile", ...)` serves map tiles from `AerialCache` and `DemCache` (GSI Japan tiles, zoom 1–11). Path format: `/{kind}/{z}/{x}/{y}` where `kind` is `aerial` or `dem`. Both `tile://localhost/…` and `https://tile.localhost/…` resolve identically because the handler reads `request.uri().path()[1..]`.
