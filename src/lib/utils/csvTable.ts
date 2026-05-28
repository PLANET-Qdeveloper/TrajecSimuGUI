export interface CsvTableData {
  headers: string[];
  rows: string[][];
}

export const TABLE_KEYS = [
  "thrust_table",
  "cp_mach_table",
  "cd0_alpha_mach_table",
  "cn_table",
  "cs_table",
  "wind_table",
  "terminal_velocity_table",
] as const;

export type TableKey = (typeof TABLE_KEYS)[number];
export type CsvTableDataMap = Record<TableKey, CsvTableData>;

export function emptyTable(): CsvTableData {
  return { headers: [], rows: [] };
}

export function defaultCsvTableDataMap(): CsvTableDataMap {
  return Object.fromEntries(TABLE_KEYS.map((k) => [k, emptyTable()])) as CsvTableDataMap;
}

export function parseCsv(text: string): CsvTableData {
  const lines = text.trim().split(/\r?\n/).filter((l) => l.length > 0);
  if (lines.length === 0) return emptyTable();
  const headers = lines[0].split(",");
  const rows = lines.slice(1).map((line) => line.split(","));
  return { headers, rows };
}

export function serializeCsv(data: CsvTableData): string {
  if (data.headers.length === 0) return "";
  return [data.headers.join(","), ...data.rows.map((r) => r.join(","))].join("\n");
}
