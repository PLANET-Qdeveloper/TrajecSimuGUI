import type { AppConfig } from "$lib/types/config";

/** Rust の google_sheets::SheetConfig に対応する型 */
export interface SheetConfig {
  // Launch
  latitude: number | null;
  longitude: number | null;
  elevation: number | null;
  rail_length: number | null;
  pitch: number | null;
  yaw: number | null;
  wind_power_exponent: number | null;
  wind_reference_alt: number | null;

  // Body（mm → m 変換済み）
  diameter_m: number | null;
  dry_mass_kg: number | null;
  cg_axial_m: number | null;
  inertia_pitch_yaw: number | null;
  inertia_roll: number | null;

  // Aero（mm → m 変換済み）
  cp_axial_m: number | null;
  roll_damping: number | null;
  pitch_damping: number | null;

  // Engine / Tank / Fuel
  oxidizer_mass_kg: number | null;
  fuel_mass_initial_kg: number | null;
  fuel_mass_final_kg: number | null;
  tank_axial_pos_m: number | null;
  fuel_axial_pos_m: number | null;

  // Parachute
  deploy_delay_sec: number | null;
}

/**
 * スプレッドシートから取得した値を既存設定にマージする。
 *
 * - スカラー値のみ更新する（ファイルパスは一切触らない）
 * - null のフィールドは無視する（既存値を保持）
 * - 3D ベクトルのうち軸方向（index 0）のみ更新する
 */
export function mergeSheetConfig(
  existing: AppConfig,
  sheet: SheetConfig,
): AppConfig {
  // structuredClone は Svelte 5 の Reactive Proxy をクローンできないため JSON ラウンドトリップを使う
  const cfg: AppConfig = JSON.parse(JSON.stringify(existing));

  // ── Launch ────────────────────────────────────────────────────────────────
  if (sheet.latitude !== null) cfg.launch.latitude = sheet.latitude;
  if (sheet.longitude !== null) cfg.launch.longitude = sheet.longitude;
  if (sheet.elevation !== null) cfg.launch.elevation = sheet.elevation;
  if (sheet.rail_length !== null) cfg.launch.rail_length = sheet.rail_length;
  if (sheet.pitch !== null) cfg.launch.pitch = sheet.pitch;
  if (sheet.yaw !== null) cfg.launch.yaw = sheet.yaw;
  if (sheet.wind_power_exponent !== null)
    cfg.launch.wind_power_exponent = sheet.wind_power_exponent;
  if (sheet.wind_reference_alt !== null)
    cfg.launch.wind_reference_alt = sheet.wind_reference_alt;

  // ── Body ──────────────────────────────────────────────────────────────────
  if (sheet.diameter_m !== null) cfg.body.diameter = sheet.diameter_m;
  if (sheet.dry_mass_kg !== null)
    cfg.body.dry_mass_with_fuel_section = sheet.dry_mass_kg;

  // cg[0] = 軸方向（機体後端からの距離）のみ更新
  if (sheet.cg_axial_m !== null) {
    cfg.body.cg = [sheet.cg_axial_m, cfg.body.cg[1], cfg.body.cg[2]];
  }

  // inertia: [Ixx(roll), Iyy(pitch), Izz(yaw), Ixy, Ixz, Iyz]
  // ピッチ・ヨーは対称のため Iyy と Izz 両方を更新する。クロス項は維持。
  if (sheet.inertia_roll !== null || sheet.inertia_pitch_yaw !== null) {
    cfg.body.inertia = [
      sheet.inertia_roll ?? cfg.body.inertia[0],
      sheet.inertia_pitch_yaw ?? cfg.body.inertia[1],
      sheet.inertia_pitch_yaw ?? cfg.body.inertia[2],
      cfg.body.inertia[3],
      cfg.body.inertia[4],
      cfg.body.inertia[5],
    ];
  }

  // ── Engine ────────────────────────────────────────────────────────────────
  // thrust_table は絶対に触らない

  if (sheet.oxidizer_mass_kg !== null)
    cfg.engine.tank.tank_contents = sheet.oxidizer_mass_kg;

  if (sheet.tank_axial_pos_m !== null) {
    cfg.engine.tank.position = [
      sheet.tank_axial_pos_m,
      cfg.engine.tank.position[1],
      cfg.engine.tank.position[2],
    ];
  }

  if (sheet.fuel_mass_initial_kg !== null) {
    cfg.engine.fuel.fuel_section_weight = sheet.fuel_mass_initial_kg;
  }
  if (sheet.fuel_mass_final_kg !== null) {
    cfg.engine.fuel.fuel_section_weight_after_burn = sheet.fuel_mass_final_kg;
  }

  if (sheet.fuel_axial_pos_m !== null) {
    cfg.engine.fuel.position = [
      sheet.fuel_axial_pos_m,
      cfg.engine.fuel.position[1],
      cfg.engine.fuel.position[2],
    ];
  }

  // ── Aero ──────────────────────────────────────────────────────────────────
  // cp_mach_table / cd0_alpha_mach_table 等のパスは触らない

  if (sheet.cp_axial_m !== null) {
    cfg.aero.cp_at_launch = [
      sheet.cp_axial_m,
      cfg.aero.cp_at_launch[1],
      cfg.aero.cp_at_launch[2],
    ];
  }

  if (sheet.roll_damping !== null) cfg.aero.roll_damping = sheet.roll_damping;
  if (sheet.pitch_damping !== null) {
    cfg.aero.pitch_damping = sheet.pitch_damping;
    cfg.aero.yaw_damping = sheet.pitch_damping; // ピッチ・ヨー対称
  }

  // ── Parachute ─────────────────────────────────────────────────────────────
  // terminal_velocity_table はファイルパスのため触らない。
  // deploy_delay_sec はパラシュートセクションが既に存在する場合のみ更新する。
  if (sheet.deploy_delay_sec !== null && cfg.parachute) {
    cfg.parachute.deploy_delay_sec = sheet.deploy_delay_sec;
  }

  return cfg;
}

/** 取込んだフィールド数をカウントする（ユーザー向け表示用） */
export function countUpdatedFields(sheet: SheetConfig): number {
  return Object.values(sheet).filter((v) => v !== null).length;
}
