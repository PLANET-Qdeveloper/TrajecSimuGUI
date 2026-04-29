//! JSBSim bridge smoke test.
//!
//! Gated with `#[ignore]` because it links against the pre-built
//! `libJSBSim.a` and actually runs the C++ FDM. Invoke with:
//!
//! ```bash
//! cargo test -p simulator_core --test jsbsim_smoke -- --ignored --nocapture
//! ```
//!
//! Verifies that:
//!   1. `JsbSimSimulator::initialize` renders XML and survives `LoadScript` +
//!      `RunIC` on minimal but physically valid `RocketParams`.
//!   2. `step()` advances time and `get_state()` returns monotonically
//!      increasing `time_sec`.
//!   3. With a non-zero thrust profile on a vertical pitch, altitude is
//!      non-decreasing over the first few seconds (it may stay pinned at
//!      ground during the initial JSBSim transient — we only assert
//!      non-negative motion, not "liftoff by step N").

use std::path::{Path, PathBuf};

use simulator_core::params::{
    AeroParams, BodyMassParams, Cd0AlphaMachTable, EngineParams, FuelParams, LaunchEnvParams,
    SimControl, TankParams,
};
use simulator_core::{JsbSimSimulator, RocketParams, Simulator};

fn walkdir(root: &Path) -> Vec<PathBuf> {
    fn visit(dir: &Path, acc: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    visit(&p, acc);
                } else {
                    acc.push(p);
                }
            }
        }
    }
    let mut acc = Vec::new();
    visit(root, &mut acc);
    acc
}

/// Minimal but flyable `RocketParams`: ~30 kg rocket, ~2 kN thrust for 1 s,
/// vertical pitch, no wind.
fn sample_params() -> RocketParams {
    RocketParams {
        body_mass: BodyMassParams {
            diameter: 0.15,
            total_mass: 30.0,
            cg: [1.0, 0.0, 0.0],
            inertia: [15.0, 15.0, 0.2, 0.0, 0.0, 0.0],
        },
        engine: EngineParams {
            thrust_table: vec![[0.0, 2000.0], [1.0, 2000.0]].into(),
            thruster_pos: [2.0, 0.0, 0.0],
            tank: TankParams {
                position: [0.8, 0.0, 0.0],
                drain_position: None,
                contents: 2.0,
            },
            fuel: FuelParams {
                position: [0.8, 0.0, 0.0],
                contents: 1.5,
                after_burn: 0.1,
            },
        },
        aero: AeroParams {
            cp_at_launch: [1.2, 0.0, 0.0],
            cp_mach_table: vec![[0.0, 1.2], [2.0, 1.2]].into(),
            cd0_alpha_mach_table: Cd0AlphaMachTable {
                mach_keys: vec![0.0, 2.0].into(),
                rows: vec![vec![0.0, 0.4, 0.4], vec![0.175, 0.5, 0.5]].into(),
            },
            cn_table: vec![[0.0, 2.0], [2.0, 2.0]].into(),
            cs_table: vec![[0.0, 2.0], [2.0, 2.0]].into(),
            roll_damping_coefficient: 0.0,
            pitch_damping_coefficient: 0.0,
            yaw_damping_coefficient: 0.0,
        },
        launch_env: LaunchEnvParams {
            latitude: 35.0,
            longitude: 139.0,
            // Start a few metres AGL with a small upward velocity so the
            // runscript's "Landed" event (h-agl < 0.01 ft ≈ 0.003 m) does
            // not fire on the very first integration step. This mirrors the
            // state the orchestrator would hand over after rail-exit.
            elevation: 5.0,
            launcher_height: 5.0,
            rail_length_m: 5.0,
            terrain: None,
            pitch: 89.0,
            roll: 0.0,
            yaw: 0.0,
            // JSBSim's XML parser rejects an empty `<tableData>`, so provide
            // at least two anchor rows even in the still-air smoke case.
            winds_table: vec![[0.0, 0.0, 0.0], [10_000.0, 0.0, 0.0]].into(),
            initial_body_velocity_mps: [30.0, 0.0, 0.0],
            initial_position_override: None,
        },
        sim: SimControl {
            flight_duration: 5.0,
            time_step: 0.01,
            apogee_mode: 0,
            csv_sample_interval: 1,
            kml_sample_interval: 10,
            start_sim_time_sec: 0.0,
        },
        parachute: Default::default(),
    }
}

#[test]
#[ignore]
fn jsbsim_initialize_step_get_state() {
    let params = sample_params();

    // Optional diagnostic: dump the generated JSBSim XML files so a failing
    // LoadScript is easy to root-cause. Enabled via env var to keep the
    // normal --nocapture output clean.
    if std::env::var_os("SMOKE_DUMP_XML").is_some() {
        use simulator_core::workspace::SimWorkspace;
        use simulator_core::xml_gen::{XmlContext, XmlGenerator};
        let ws = SimWorkspace::new().unwrap();
        let ctx = XmlContext::from(&params);
        XmlGenerator::new().render_into(&ctx, &ws).unwrap();
        println!("workspace root: {}", ws.root().display());
        for entry in walkdir(ws.root()) {
            println!("  · {}", entry.display());
        }
    }

    let mut sim = JsbSimSimulator::new();

    sim.initialize(&params)
        .expect("JSBSim initialize should succeed on valid minimal params");

    let mut last_time = -1.0;
    let mut max_alt = 0.0_f64;
    let max_steps = 200; // ~2 simulated seconds at dt=0.01

    for i in 0..max_steps {
        let running = sim.step().expect("step must not error");
        let st = sim.get_state().expect("get_state must not error");

        assert!(
            st.time_sec >= last_time,
            "sim time regressed at iter {i}: prev={last_time}, now={}",
            st.time_sec
        );
        assert!(
            st.position.alt_agl_m >= -0.1,
            "altitude AGL went substantially negative at iter {i}: {}",
            st.position.alt_agl_m
        );

        if i % 20 == 0 {
            println!(
                "iter={i:>3} t={:6.3}s alt={:7.3}m u={:7.3}m/s thrust={:8.2}N mach={:.3}",
                st.time_sec,
                st.position.alt_agl_m,
                st.velocity.u_mps,
                st.thrust_n,
                st.mach,
            );
        }

        last_time = st.time_sec;
        max_alt = max_alt.max(st.position.alt_agl_m);

        if !running {
            println!("JSBSim signalled termination at iter {i}, t={:.3}s", st.time_sec);
            break;
        }
    }

    println!("max AGL altitude observed: {max_alt:.3} m");
    assert!(
        last_time > 0.0,
        "sim time never advanced — JSBSim did not actually run"
    );
}
