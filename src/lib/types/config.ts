export interface LaunchConfig {
  latitude: number;
  longitude: number;
  elevation: number;
  rail_length: number;
  pitch: number;
  roll: number;
  yaw: number;
  wind_speed_mps?: number;
  wind_direction_deg?: number;
  wind_reference_alt?: number;
  wind_power_exponent: number;
  wind_table?: string;
}

export interface BodyConfig {
  diameter: number;
  dry_mass_with_fuel_section: number;
  cg: [number, number, number];
  inertia: [number, number, number, number, number, number];
}

export interface TankConfig {
  position: [number, number, number];
  drain_position?: [number, number, number];
  tank_contents: number;
}

export interface FuelConfig {
  position: [number, number, number];
  fuel_section_weight: number;
  fuel_section_weight_after_burn: number;
}

export interface EngineConfig {
  thrust_table: string;
  thruster_pos: [number, number, number];
  tank: TankConfig;
  fuel: FuelConfig;
}

export interface AeroConfig {
  cp_at_launch: [number, number, number];
  cp_mach_table: string;
  cd0_alpha_mach_table: string;
  cn_table: string;
  cs_table: string;
  roll_damping: number;
  pitch_damping: number;
  yaw_damping: number;
}

export interface ParachuteConfig {
  terminal_velocity_table: string;
  deploy_delay_sec: number;
}

export interface SimConfig {
  flight_duration: number;
  time_step: number;
  csv_sample_interval: number;
  kml_sample_interval: number;
}

export interface AppConfig {
  launch: LaunchConfig;
  body: BodyConfig;
  engine: EngineConfig;
  aero: AeroConfig;
  parachute?: ParachuteConfig;
  sim: SimConfig;
}

export enum TelemetryDataKey {
  // ── Position ────────────────────────────────────────────────────────────
  LatDeg = "lat_deg",
  LonDeg = "lon_deg",
  AltMslM = "alt_msl_m",
  DownRangeM = "down_range_m",
  LocalXM = "local_x_m",
  LocalYM = "local_y_m",

  // ── Velocity ────────────────────────────────────────────────────────────
  UMps = "u_mps",
  VMps = "v_mps",
  WMps = "w_mps",
  TrueAirspeedMps = "true_airspeed_mps",
  GroundSpeedMps = "ground_speed_mps",

  // ── Attitude ────────────────────────────────────────────────────────────
  PitchDeg = "pitch_deg",
  RollDeg = "roll_deg",
  YawDeg = "yaw_deg",

  // ── Angular rates ────────────────────────────────────────────────────────
  PRadSec = "p_rad_sec",
  QRadSec = "q_rad_sec",
  RRadSec = "r_rad_sec",

  // ── Acceleration ─────────────────────────────────────────────────────────
  AxMps2 = "ax_mps2",
  AyMps2 = "ay_mps2",
  AzMps2 = "az_mps2",

  // ── Aerodynamics / atmosphere ────────────────────────────────────────────
  AlphaDeg = "alpha_deg",
  BetaDeg = "beta_deg",
  QbarPa = "qbar_pa",
  TotalAoaDeg = "total_aoa_deg",
  PressurePa = "pressure_pa",
  TemperatureK = "temperature_k",
  GustAirspeedMps = "gust_airspeed_mps",
  GustAoaDeg = "gust_aoa_deg",

  // ── Propulsion ───────────────────────────────────────────────────────────
  ThrustN = "thrust_n",
  Mach = "mach",
}

export interface Trajectory {
  time_sec: number[];

  // ── Position ────────────────────────────────────────────────────────────
  lat_deg: number[];
  lon_deg: number[];
  alt_msl_m: number[];
  down_range_m: number[];
  local_x_m: number[];
  local_y_m: number[];

  // ── Velocity ────────────────────────────────────────────────────────────
  u_mps: number[];
  v_mps: number[];
  w_mps: number[];
  true_airspeed_mps: number[];
  ground_speed_mps: number[];

  // ── Attitude ────────────────────────────────────────────────────────────
  pitch_deg: number[];
  roll_deg: number[];
  yaw_deg: number[];

  // ── Angular rates ────────────────────────────────────────────────────────
  p_rad_sec: number[];
  q_rad_sec: number[];
  r_rad_sec: number[];

  // ── Acceleration ─────────────────────────────────────────────────────────
  ax_mps2: number[];
  ay_mps2: number[];
  az_mps2: number[];

  // ── Aerodynamics / atmosphere ────────────────────────────────────────────
  alpha_deg: number[];
  beta_deg: number[];
  qbar_pa: number[];
  total_aoa_deg: number[];
  pressure_pa: number[];
  temperature_k: number[];
  gust_airspeed_mps: number[];
  gust_aoa_deg: number[];

  // ── Propulsion ───────────────────────────────────────────────────────────
  thrust_n: number[];
  mach: number[];
}

export interface SimSummary {
  apogee_m: number;
  max_speed_mps: number;
  flight_time_sec: number;
  landing_lat_parachute?: number;
  landing_lon_parachute?: number;
  landing_alt_m_parachute?: number;
  landing_lat_ballistic?: number;
  landing_lon_ballistic?: number;
  landing_alt_m_ballistic?: number;
  trajectory_ballistic: Trajectory;
  trajectory_parachute: Trajectory;
  kml_result: string;
  out_dir: string;
}

export interface LandingAreaSummary {
  out_dir: string;
  kml_result: string;
}

export function defaultConfig(): AppConfig {
  return {
    launch: {
      latitude: 0,
      longitude: 0,
      elevation: 0,
      rail_length: 5.0,
      pitch: 89.0,
      roll: 0.0,
      yaw: 0.0,
      wind_speed_mps: undefined,
      wind_direction_deg: undefined,
      wind_reference_alt: undefined,
      wind_power_exponent: 0.166666666,
      wind_table: undefined,
    },
    body: {
      diameter: 0.1,
      dry_mass_with_fuel_section: 10.0,
      cg: [0.5, 0, 0],
      inertia: [5.0, 5.0, 0.1, 0, 0, 0],
    },
    engine: {
      thrust_table: "",
      thruster_pos: [1.0, 0, 0],
      tank: {
        position: [0.5, 0, 0],
        drain_position: undefined,
        tank_contents: 1.0,
      },
      fuel: {
        position: [0.5, 0, 0],
        fuel_section_weight: 1.0,
        fuel_section_weight_after_burn: 0.1,
      },
    },
    aero: {
      cp_at_launch: [0.6, 0, 0],
      cp_mach_table: "",
      cd0_alpha_mach_table: "",
      cn_table: "",
      cs_table: "",
      roll_damping: 0.0,
      pitch_damping: 0.0,
      yaw_damping: 0.0,
    },
    parachute: undefined,
    sim: {
      flight_duration: 120.0,
      time_step: 0.01,
      csv_sample_interval: 1,
      kml_sample_interval: 10,
    },
  };
}
