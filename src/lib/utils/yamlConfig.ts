import * as yaml from 'js-yaml';
import { type AppConfig, defaultConfig } from '$lib/types/config';
import { toAbsolute, toRelative } from '$lib/utils/path';

export function parseConfig(raw: unknown, baseDir: string): AppConfig {
  const p = raw as Record<string, unknown>;
  const def = defaultConfig();
  const abs = (v: unknown) => (typeof v === 'string' ? toAbsolute(baseDir, v) : '');

  const launch = (p.launch as Record<string, unknown>) ?? {};
  const body = (p.body as Record<string, unknown>) ?? {};
  const engine = (p.engine as Record<string, unknown>) ?? {};
  const tank = (engine.tank as Record<string, unknown>) ?? {};
  const fuel = (engine.fuel as Record<string, unknown>) ?? {};
  const aero = (p.aero as Record<string, unknown>) ?? {};
  const para = p.parachute as Record<string, unknown> | undefined;
  const sim = (p.sim as Record<string, unknown>) ?? {};

  const arr3 = (v: unknown, d: [number, number, number]): [number, number, number] =>
    Array.isArray(v) && v.length >= 3 ? [+v[0], +v[1], +v[2]] : d;
  const arr6 = (v: unknown, d: [number, number, number, number, number, number]) =>
    Array.isArray(v) && v.length >= 6
      ? ([+v[0], +v[1], +v[2], +v[3], +v[4], +v[5]] as [number, number, number, number, number, number])
      : d;
  const num = (v: unknown, d: number) => (typeof v === 'number' ? v : d);
  const opt = (v: unknown): number | undefined => (typeof v === 'number' ? v : undefined);

  return {
    launch: {
      latitude: num(launch.latitude, def.launch.latitude),
      longitude: num(launch.longitude, def.launch.longitude),
      elevation: num(launch.elevation, def.launch.elevation),
      rail_length: num(launch.rail_length, def.launch.rail_length),
      pitch: num(launch.pitch, def.launch.pitch),
      roll: num(launch.roll, def.launch.roll),
      yaw: num(launch.yaw, def.launch.yaw),
      wind_speed_mps: opt(launch.wind_speed_mps),
      wind_direction_deg: opt(launch.wind_direction_deg),
      wind_reference_alt: opt(launch.wind_reference_alt),
      wind_power_exponent: num(launch.wind_power_exponent, def.launch.wind_power_exponent),
      wind_table:
        typeof launch.wind_table === 'string' ? toAbsolute(baseDir, launch.wind_table) : undefined,
    },
    body: {
      diameter: num(body.diameter, def.body.diameter),
      dry_mass_with_fuel_section: num(
        body.dry_mass_with_fuel_section,
        def.body.dry_mass_with_fuel_section,
      ),
      cg: arr3(body.cg, def.body.cg),
      inertia: arr6(body.inertia, def.body.inertia),
    },
    engine: {
      thrust_table: abs(engine.thrust_table),
      thruster_pos: arr3(engine.thruster_pos, def.engine.thruster_pos),
      tank: {
        position: arr3(tank.position, def.engine.tank.position),
        drain_position: Array.isArray(tank.drain_position)
          ? arr3(tank.drain_position, [0, 0, 0])
          : undefined,
        tank_contents: num(tank.tank_contents, def.engine.tank.tank_contents),
      },
      fuel: {
        position: arr3(fuel.position, def.engine.fuel.position),
        fuel_section_weight: num(fuel.fuel_section_weight, def.engine.fuel.fuel_section_weight),
        fuel_section_weight_after_burn: num(
          fuel.fuel_section_weight_after_burn,
          def.engine.fuel.fuel_section_weight_after_burn,
        ),
      },
    },
    aero: {
      cp_at_launch: arr3(aero.cp_at_launch, def.aero.cp_at_launch),
      cp_mach_table: abs(aero.cp_mach_table),
      cd0_alpha_mach_table: abs(aero.cd0_alpha_mach_table),
      cn_table: abs(aero.cn_table),
      cs_table: abs(aero.cs_table),
      roll_damping: num(aero.roll_damping, def.aero.roll_damping),
      pitch_damping: num(aero.pitch_damping, def.aero.pitch_damping),
      yaw_damping: num(aero.yaw_damping, def.aero.yaw_damping),
    },
    parachute: para
      ? {
          terminal_velocity_table: abs(para.terminal_velocity_table),
          deploy_delay_sec: num(para.deploy_delay_sec, 1.0),
        }
      : undefined,
    sim: {
      flight_duration: num(sim.flight_duration, def.sim.flight_duration),
      time_step: num(sim.time_step, def.sim.time_step),
      csv_sample_interval: num(sim.csv_sample_interval, def.sim.csv_sample_interval),
      kml_sample_interval: num(sim.kml_sample_interval, def.sim.kml_sample_interval),
    },
  };
}

export function serializeConfig(config: AppConfig, baseDir: string): string {
  const rel = (p: string) => (baseDir && p ? toRelative(baseDir, p) : p);

  const out: Record<string, unknown> = {
    launch: {
      latitude: config.launch.latitude,
      longitude: config.launch.longitude,
      elevation: config.launch.elevation,
      rail_length: config.launch.rail_length,
      pitch: config.launch.pitch,
      roll: config.launch.roll,
      yaw: config.launch.yaw,
      ...(config.launch.wind_table
        ? { wind_table: rel(config.launch.wind_table) }
        : {
            ...(config.launch.wind_speed_mps !== undefined
              ? { wind_speed_mps: config.launch.wind_speed_mps }
              : {}),
            ...(config.launch.wind_direction_deg !== undefined
              ? { wind_direction_deg: config.launch.wind_direction_deg }
              : {}),
            ...(config.launch.wind_reference_alt !== undefined
              ? { wind_reference_alt: config.launch.wind_reference_alt }
              : {}),
            wind_power_exponent: config.launch.wind_power_exponent,
          }),
    },
    body: {
      diameter: config.body.diameter,
      dry_mass_with_fuel_section: config.body.dry_mass_with_fuel_section,
      cg: config.body.cg,
      inertia: config.body.inertia,
    },
    engine: {
      thrust_table: rel(config.engine.thrust_table),
      thruster_pos: config.engine.thruster_pos,
      tank: {
        position: config.engine.tank.position,
        ...(config.engine.tank.drain_position
          ? { drain_position: config.engine.tank.drain_position }
          : {}),
        tank_contents: config.engine.tank.tank_contents,
      },
      fuel: {
        position: config.engine.fuel.position,
        fuel_section_weight: config.engine.fuel.fuel_section_weight,
        fuel_section_weight_after_burn: config.engine.fuel.fuel_section_weight_after_burn,
      },
    },
    aero: {
      cp_at_launch: config.aero.cp_at_launch,
      cp_mach_table: rel(config.aero.cp_mach_table),
      cd0_alpha_mach_table: rel(config.aero.cd0_alpha_mach_table),
      cn_table: rel(config.aero.cn_table),
      cs_table: rel(config.aero.cs_table),
      roll_damping: config.aero.roll_damping,
      pitch_damping: config.aero.pitch_damping,
      yaw_damping: config.aero.yaw_damping,
    },
    sim: {
      flight_duration: config.sim.flight_duration,
      time_step: config.sim.time_step,
      csv_sample_interval: config.sim.csv_sample_interval,
      kml_sample_interval: config.sim.kml_sample_interval,
    },
  };

  if (config.parachute) {
    out.parachute = {
      terminal_velocity_table: rel(config.parachute.terminal_velocity_table),
      deploy_delay_sec: config.parachute.deploy_delay_sec,
    };
  }

  return yaml.dump(out, { noRefs: true, lineWidth: -1 });
}
