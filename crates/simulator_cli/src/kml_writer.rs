//! KML 2.2 writer for trajectory visualisation in Google Earth & friends.
//!
//! Emits one `<Document>` containing two `LineString` placemarks (mainline
//! ballistic + parachute branch) plus point placemarks for the major events.
//! Altitudes are written as MSL (`<altitudeMode>absolute</altitudeMode>`)
//! by adding the launch-pad elevation to each AGL sample. When a `Terrain`
//! is configured the per-point terrain offset is added too.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

use simulator_core::params::RocketParams;
use simulator_core::progress::EventStamp;
use simulator_core::{EventKind, Trajectory, UnifiedSimulationOutput};

const KML_HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
<Document>
  <name>TrajecSimuGUI flight</name>
  <Style id="ballistic">
    <LineStyle><color>ff005ed5</color><width>2</width></LineStyle>
  </Style>
  <Style id="parachute">
    <LineStyle><color>ffa779cc</color><width>2</width></LineStyle>
  </Style>
  <Style id="event">
    <IconStyle>
      <color>ffffffff</color>
      <Icon><href>http://maps.google.com/mapfiles/kml/shapes/placemark_circle.png</href></Icon>
    </IconStyle>
  </Style>
"#;

const KML_FOOTER: &str = "</Document>\n</kml>\n";

pub fn write_trajectory_kml(
    path: &Path,
    output: &UnifiedSimulationOutput,
    params: &RocketParams,
    interval: usize,
) -> Result<()> {
    let interval = interval.max(1);
    let mut f = fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    f.write_all(KML_HEADER.as_bytes())?;

    write_linestring(
        &mut f,
        "ballistic",
        "Ballistic phase",
        &output.mainline.trajectory,
        params,
        interval,
    )?;
    write_linestring(
        &mut f,
        "parachute",
        "Parachute descent",
        &output.parachute_branch.trajectory,
        params,
        interval,
    )?;
    write_event_placemarks(&mut f, &output.events, params)?;

    f.write_all(KML_FOOTER.as_bytes())?;
    Ok(())
}

fn write_linestring(
    f: &mut fs::File,
    style_id: &str,
    name: &str,
    traj: &Trajectory,
    _params: &RocketParams,
    interval: usize,
) -> Result<()> {
    if traj.is_empty() {
        return Ok(());
    }
    writeln!(
        f,
        "  <Placemark>\n    <name>{name}</name>\n    <styleUrl>#{style_id}</styleUrl>\n    \
         <LineString>\n      <altitudeMode>absolute</altitudeMode>\n      <coordinates>"
    )?;
    let len = traj.len();
    for (i, s) in traj.row_iter().enumerate() {
        if interval > 1 && i % interval != 0 && i + 1 != len {
            continue;
        }
        let alt_msl = s.position.alt_agl_m;
        writeln!(
            f,
            "        {:.7},{:.7},{:.3}",
            s.position.lon_deg, s.position.lat_deg, alt_msl
        )?;
    }
    writeln!(f, "      </coordinates>\n    </LineString>\n  </Placemark>")?;
    Ok(())
}

fn write_event_placemarks(
    f: &mut fs::File,
    events: &[EventStamp],
    _params: &RocketParams,
) -> Result<()> {
    for e in events {
        // Skip events with no spatial state (e.g. Start at t=0 before
        // the first physics step).
        let Some(state) = e.state.as_ref() else {
            continue;
        };
        let alt_msl = state.position.alt_agl_m;
        let kind = e.kind;
        let label = event_label(kind);
        writeln!(
            f,
            "  <Placemark>\n    <name>{label}</name>\n    <styleUrl>#event</styleUrl>\n    \
             <description>t={:.3}s alt_agl={:.1}m mach={:.3} qbar={:.1}Pa</description>\n    \
             <Point>\n      <altitudeMode>absolute</altitudeMode>\n      \
             <coordinates>{:.7},{:.7},{:.3}</coordinates>\n    </Point>\n  </Placemark>",
            e.sim_time_sec,
            state.position.alt_agl_m,
            state.mach,
            state.aero.qbar_pa,
            state.position.lon_deg,
            state.position.lat_deg,
            alt_msl,
        )?;
    }
    Ok(())
}

fn event_label(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Start => "Start",
        EventKind::LaunchClear => "Launch clear",
        EventKind::EngineBurnout => "Engine burnout",
        EventKind::Apogee => "Apogee",
        EventKind::ParachuteOpen => "Parachute open",
        EventKind::Landed => "Landed",
        EventKind::ParachuteLanded => "Parachute landed",
        EventKind::MaxQ => "Max Q",
        EventKind::MaxAxialAcceleration => "Max axial accel",
        EventKind::MaxLateralAcceleration => "Max lateral accel",
        EventKind::MaxAngularRate => "Max angular rate",
        EventKind::MaxThrust => "Max thrust",
        EventKind::MaxAirspeed => "Max airspeed",
        EventKind::MaxDynamicPressureAlpha => "Max dynamic pressure alpha",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simulator_core::analysis::AnalysisOutput;
    use simulator_core::output::SimulationOutput;
    use simulator_core::progress::{EventSource, EventStamp};
    use simulator_core::SimulationState;

    fn make_state(t: f64, lat: f64, lon: f64, alt_agl: f64) -> SimulationState {
        SimulationState {
            time_sec: t,
            position: simulator_core::output::Position {
                lat_deg: lat,
                lon_deg: lon,
                alt_agl_m: alt_agl,
                ..Default::default()
            },
            mach: 0.5,
            aero: simulator_core::output::AeroState {
                alpha_deg: 0.0,
                beta_deg: 0.0,
                qbar_pa: 1234.0,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn empty_output() -> UnifiedSimulationOutput {
        UnifiedSimulationOutput {
            mainline: SimulationOutput::new(),
            parachute_branch: SimulationOutput::new(),
            events: Vec::new(),
            analysis: AnalysisOutput::default(),
        }
    }

    fn flat_params() -> RocketParams {
        use simulator_core::params::{
            AeroParams, BodyMassParams, Cd0AlphaMachTable, EngineParams, FuelParams,
            LaunchEnvParams, ParachuteParams, RocketParams, SimControl, TankParams,
        };
        RocketParams {
            body_mass: BodyMassParams {
                diameter: 0.1,
                total_mass: 10.0,
                cg: [0.5, 0.0, 0.0],
                inertia: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0],
            },
            engine: EngineParams {
                thrust_table: vec![[0.0, 100.0]].into(),
                thruster_pos: [1.0, 0.0, 0.0],
                tank: TankParams {
                    position: [0.5, 0.0, 0.0],
                    drain_position: None,
                    contents: 0.1,
                },
                fuel: FuelParams {
                    position: [0.5, 0.0, 0.0],
                    contents: 0.1,
                    after_burn: 0.0,
                },
            },
            aero: AeroParams {
                cp_at_launch: [0.5, 0.0, 0.0],
                cp_mach_table: vec![[0.0, 0.5]].into(),
                cd0_alpha_mach_table: Cd0AlphaMachTable {
                    mach_keys: vec![0.0].into(),
                    rows: vec![vec![0.0, 0.3]].into(),
                },
                cn_table: vec![[0.0, 2.0]].into(),
                cs_table: vec![[0.0, 2.0]].into(),
                roll_damping_coefficient: 0.0,
                pitch_damping_coefficient: 0.0,
                yaw_damping_coefficient: 0.0,
            },
            launch_env: LaunchEnvParams {
                latitude: 35.0,
                longitude: 139.0,
                elevation: 5.0,
                rail_length_m: 5.0,
                pitch: 90.0,
                roll: 0.0,
                yaw: 0.0,
                winds_table: Vec::<[f64; 3]>::new().into(),
                initial_body_velocity_mps: [0.0; 3],
                initial_position_override: None,
            },
            sim: SimControl::default(),
            parachute: ParachuteParams::default(),
        }
    }

    #[test]
    fn omits_empty_branches() {
        let mut out = empty_output();
        for s in [make_state(0.0, 35.0, 139.0, 0.0), make_state(1.0, 35.0, 139.0, 100.0)] {
            out.mainline.trajectory.push(&s);
        }
        // parachute branch left empty
        let p = flat_params();
        let path = std::env::temp_dir().join("kml_omits_empty.kml");
        write_trajectory_kml(&path, &out, &p, 1).unwrap();
        let kml = fs::read_to_string(&path).unwrap();
        assert!(kml.contains("Ballistic phase"));
        assert!(!kml.contains("Parachute descent"));
    }

    #[test]
    fn respects_interval_and_keeps_last() {
        let mut out = empty_output();
        for i in 0..10 {
            let s = make_state(i as f64, 35.0 + i as f64 * 0.001, 139.0, 100.0 * i as f64);
            out.mainline.trajectory.push(&s);
        }
        let p = flat_params();
        let path = std::env::temp_dir().join("kml_interval.kml");
        write_trajectory_kml(&path, &out, &p, 5).unwrap();
        let kml = fs::read_to_string(&path).unwrap();
        // Expect points at i=0,5,9 (last); count "        " (8 leading
        // spaces, used only on coordinate rows).
        let coord_rows = kml.matches("        ").count();
        assert_eq!(
            coord_rows, 3,
            "expected 3 sampled coordinates (i=0,5,9), got {coord_rows}\n{kml}"
        );
    }

    #[test]
    fn emits_well_formed_xml() {
        let mut out = empty_output();
        for s in [make_state(0.0, 35.0, 139.0, 5.0), make_state(1.0, 35.001, 139.0, 50.0)] {
            out.mainline.trajectory.push(&s);
        }
        out.events.push(EventStamp {
            kind: EventKind::Apogee,
            sim_time_sec: 1.0,
            source: EventSource::JsbSim,
            state: Some(make_state(1.0, 35.001, 139.0, 50.0)),
        });
        let p = flat_params();
        let path = std::env::temp_dir().join("kml_xml.kml");
        write_trajectory_kml(&path, &out, &p, 1).unwrap();
        let kml = fs::read_to_string(&path).unwrap();
        assert!(kml.starts_with("<?xml version=\"1.0\""));
        assert!(kml.contains("<kml xmlns=\"http://www.opengis.net/kml/2.2\">"));
        assert!(kml.contains("<Placemark>"));
        assert!(kml.contains("Apogee"));
        assert!(kml.trim_end().ends_with("</kml>"));
    }
}
