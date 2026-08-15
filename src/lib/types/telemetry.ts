// 実機テレメトリ (NSE2026, ref/NSE2026_PROTOCOL.md) 用の型定義。
// Rust側 (src-tauri/src/serial.rs) が送出するイベントペイロードと1:1対応する。

export type SerialFormat = "byte" | "csv";

/** シリアルポートへ割り当てるデータの種類。Rust側の `kind` 文字列と一致させる。 */
export enum DataKind {
  Altitude = "altitude",
  Latitude = "latitude",
  Longitude = "longitude",
  Pressure = "pressure",
  // バックエンドが Pressure から自動算出する派生値。マッピングの割当先には
  // 使わないが、表示側では values の1キーとして参照する。
  PressureAltitude = "pressure_altitude",
  Temperature = "temperature",
  Phase = "phase",
  ElapsedTimeMs = "elapsed_time_ms",
  Voltage = "voltage",
  Current = "current",
  AccelX = "accel_x",
  AccelY = "accel_y",
  AccelZ = "accel_z",
  NumSats = "num_sats",
  FlagCanErrValve = "flag_can_err_valve",
  FlagCanErrSep = "flag_can_err_sep",
  Nos = "nos",
  Ignore = "ignore",
}

export const dataKindLabel: Record<DataKind, string> = {
  [DataKind.Altitude]: "高度",
  [DataKind.Latitude]: "緯度",
  [DataKind.Longitude]: "経度",
  [DataKind.Pressure]: "気圧",
  [DataKind.PressureAltitude]: "気圧高度",
  [DataKind.Temperature]: "温度",
  [DataKind.Phase]: "フェーズ",
  [DataKind.ElapsedTimeMs]: "経過時間",
  [DataKind.Voltage]: "電圧",
  [DataKind.Current]: "電流",
  [DataKind.AccelX]: "X加速度",
  [DataKind.AccelY]: "Y加速度",
  [DataKind.AccelZ]: "Z加速度",
  [DataKind.NumSats]: "GNSS衛星数",
  [DataKind.FlagCanErrValve]: "バルブCAN生存",
  [DataKind.FlagCanErrSep]: "分離系CAN生存",
  [DataKind.Nos]: "NOS",
  [DataKind.Ignore]: "(未割当)",
};

export const dataKindOptions = Object.values(DataKind).map((value) => ({
  value,
  label: dataKindLabel[value],
}));

export interface ByteTagMapping {
  /** 2桁16進文字列, 例 "A1" */
  tag: string;
  kind: DataKind;
}

export interface CsvColumnMapping {
  /** 0始まりの列番号 */
  index: number;
  kind: DataKind;
}

/** メイン基板テレメトリ (ref/NSE2026_PROTOCOL.md 「メイン基板テレメトリ」) の既定タグ割当。 */
export function defaultMainBoardByteMapping(): ByteTagMapping[] {
  return [
    { tag: "A1", kind: DataKind.Altitude },
    { tag: "A2", kind: DataKind.Latitude },
    { tag: "A3", kind: DataKind.Longitude },
    { tag: "A4", kind: DataKind.Pressure },
    { tag: "A5", kind: DataKind.Temperature },
    { tag: "A6", kind: DataKind.Phase },
    { tag: "A7", kind: DataKind.Ignore },
    { tag: "A8", kind: DataKind.ElapsedTimeMs },
    { tag: "A9", kind: DataKind.Voltage },
    { tag: "AA", kind: DataKind.Current },
    { tag: "AB", kind: DataKind.AccelX },
    { tag: "AC", kind: DataKind.AccelY },
    { tag: "AD", kind: DataKind.AccelZ },
    { tag: "AE", kind: DataKind.NumSats },
    { tag: "AF", kind: DataKind.FlagCanErrValve },
    { tag: "B0", kind: DataKind.FlagCanErrSep },
  ];
}

/** バルブ基板テレメトリ (ref/NSE2026_PROTOCOL.md 「バルブ基板テレメトリ」) の既定列割当。 */
export function defaultValveBoardCsvMapping(): CsvColumnMapping[] {
  return [
    { index: 0, kind: DataKind.ElapsedTimeMs },
    { index: 1, kind: DataKind.Phase },
    { index: 2, kind: DataKind.Pressure },
    { index: 3, kind: DataKind.Voltage },
    { index: 4, kind: DataKind.Current },
    { index: 5, kind: DataKind.Temperature },
    { index: 6, kind: DataKind.Nos },
  ];
}

export type SerialPortPreset = "main" | "valve" | "custom";

export interface SerialPortSlotConfig {
  id: string;
  label: string;
  enabled: boolean;
  portPath: string;
  baudRate: number;
  format: SerialFormat;
  preset: SerialPortPreset;
  /** 生気圧値の単位。バルブ基板のCSV気圧列は仕様上kPaだが、バイナリ同様に変更可能。 */
  pressureUnit: "pa" | "hpa" | "kpa";
  seaLevelPressurePa: number;
  byteMapping: ByteTagMapping[];
  csvMapping: CsvColumnMapping[];
  /** バイトフォーマットのフレームフィルタ (2桁16進, 例 "0B")。既定はメイン基板の
   * 宛先/送信元ID。「カスタム」プリセットで他のバイトフレーム機器に合わせて
   * 変更できる。 */
  destinationId: string;
  sourceId: string;
  /** 接続中、受信データ（生バイト列+デコード済みCSV）をDownloadsフォルダへ保存する。 */
  record: boolean;
}

/** メイン基板テレメトリの既定の宛先/送信元ID (ref/NSE2026_PROTOCOL.md)。 */
export const DEFAULT_MAIN_DESTINATION_ID = "0B";
export const DEFAULT_MAIN_SOURCE_ID = "0A";

export function defaultSerialPortSlotConfig(
  id: string,
  preset: SerialPortPreset = "custom",
): SerialPortSlotConfig {
  const isMain = preset === "main";
  const isValve = preset === "valve";
  return {
    id,
    label: isMain ? "メイン基板" : isValve ? "バルブ基板" : `ポート${id}`,
    enabled: false,
    portPath: "",
    baudRate: 115200,
    format: isValve ? "csv" : "byte",
    preset,
    pressureUnit: isValve ? "kpa" : "hpa",
    seaLevelPressurePa: 101325.0,
    byteMapping: defaultMainBoardByteMapping(),
    csvMapping: defaultValveBoardCsvMapping(),
    destinationId: DEFAULT_MAIN_DESTINATION_ID,
    sourceId: DEFAULT_MAIN_SOURCE_ID,
    record: true,
  };
}

/** 4枠のうち、モック環境 (main: /dev/ttys001, valve: /dev/ttys006) にすぐ
 * 接続できるよう、スロット1=メイン既定、スロット2=バルブ既定にしておく。 */
export function defaultSerialPortSlots(): SerialPortSlotConfig[] {
  return [
    defaultSerialPortSlotConfig("1", "main"),
    defaultSerialPortSlotConfig("2", "valve"),
    defaultSerialPortSlotConfig("3", "custom"),
    defaultSerialPortSlotConfig("4", "custom"),
  ];
}

export type TelemetryValue =
  | { type: "number"; value: number }
  | { type: "text"; value: string };

export interface TelemetrySample {
  slotId: string;
  format: SerialFormat;
  recvUnixMs: number;
  values: Record<string, TelemetryValue>;
}

export interface TelemetryStatus {
  slotId: string;
  goodCount: number;
  crcErrors: number;
  lengthErrors: number;
  unknownTag: number;
  badLines: number;
  lastGoodUnixMs: number | null;
  disconnects: number;
  connected: boolean;
}

/** `start_serial_telemetry` が録画開始時に返す保存先（Downloadsフォルダ固定）。 */
export interface RecordingInfo {
  rawPath: string;
  csvPath: string;
}

export interface TelemetryRecordingError {
  slotId: string;
  message: string;
}

/** 色分け用の固定パレット。スロット(系統)ごとに固定色を割り当て、平均化はしない。 */
export const SLOT_COLORS = ["#0072b2", "#d55e00", "#009e73", "#cc79a7"];

export function slotColor(index: number): string {
  return SLOT_COLORS[index % SLOT_COLORS.length];
}

/** 地図上の軌跡・マーカー用のオレンジ系パレット。空撮画像上での視認性を優先し、
 * グラフの配色 (SLOT_COLORS) とは独立にスロットごとの濃淡で塗り分ける。 */
export const MAP_TRACK_COLORS = ["#f97316", "#c2410c", "#fdba74", "#7c2d12"];

export function mapTrackColor(index: number): string {
  return MAP_TRACK_COLORS[index % MAP_TRACK_COLORS.length];
}

/** 数値ヘルパー: TelemetryValue から数値を取り出す (文字列や欠損は undefined)。 */
export function numberValue(
  values: Record<string, TelemetryValue> | undefined,
  kind: DataKind,
): number | undefined {
  const v = values?.[kind];
  return v?.type === "number" ? v.value : undefined;
}

export function rawValue(
  values: Record<string, TelemetryValue> | undefined,
  kind: DataKind,
): string | undefined {
  const v = values?.[kind];
  if (!v) return undefined;
  return v.type === "number" ? String(v.value) : v.value;
}
