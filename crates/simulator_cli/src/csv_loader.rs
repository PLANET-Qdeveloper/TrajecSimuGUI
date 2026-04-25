//! Permissive CSV reader for aero / thrust / wind / parachute tables.
//!
//! Supports comma, tab, or whitespace delimiters. Header row is optional
//! for 1-D tables (auto-detected by whether all cells parse as `f64`).
//! Comment lines starting with `#` are skipped.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use simulator_core::params::Cd0AlphaMachTable;

/// Split a line into cells, trying `,`, then `\t`, then whitespace.
fn split_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.contains(',') {
        return trimmed
            .split(',')
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
    }
    if trimmed.contains('\t') {
        return trimmed
            .split('\t')
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
    }
    trimmed
        .split_whitespace()
        .map(|c| c.to_string())
        .collect()
}

/// Read content, strip empty and `#`-comment lines, return cell vectors.
fn tokenize(path: &Path) -> Result<Vec<Vec<String>>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading CSV file {}", path.display()))?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let cells = split_cells(line);
        if cells.is_empty() {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        bail!("CSV file {} contained no data rows", path.display());
    }
    Ok(rows)
}

fn parse_row_as_f64(row: &[String]) -> Option<Vec<f64>> {
    row.iter()
        .map(|c| c.parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()
}

/// Load a 1-D `[[x, y], ...]` table from a CSV file.
///
/// The first row is treated as a header (and dropped) if **any** cell
/// in it fails to parse as `f64`. Otherwise it is kept as data. This
/// supports the original `time,thrust` style reference CSVs as well as
/// pure-numeric tables.
pub fn load_1d(path: &Path) -> Result<Vec<[f64; 2]>> {
    let rows = tokenize(path)?;
    let mut iter = rows.into_iter();
    let first = iter.next().expect("tokenize guarantees ≥1 row");

    let mut data: Vec<[f64; 2]> = Vec::new();
    // If the first row is fully numeric, treat it as data.
    if let Some(nums) = parse_row_as_f64(&first) {
        push_2cols(&mut data, &nums, path)?;
    }
    // else: header row, drop silently.

    for row in iter {
        let nums = parse_row_as_f64(&row).ok_or_else(|| {
            anyhow!(
                "non-numeric cell in {} after header (row: {:?})",
                path.display(),
                row
            )
        })?;
        push_2cols(&mut data, &nums, path)?;
    }

    if data.is_empty() {
        bail!(
            "CSV file {} had a header but no data rows",
            path.display()
        );
    }
    Ok(data)
}

fn push_2cols(out: &mut Vec<[f64; 2]>, row: &[f64], path: &Path) -> Result<()> {
    if row.len() < 2 {
        bail!(
            "expected at least 2 columns in {}, got {} ({:?})",
            path.display(),
            row.len(),
            row
        );
    }
    out.push([row[0], row[1]]);
    Ok(())
}

/// Load a 2-D Cd table `α[deg] × Mach` and convert to `Cd0AlphaMachTable`.
///
/// Layout:
/// - **Row 0 (required header)**: first cell is an arbitrary label
///   (e.g. `"alpha/mach"`), remaining cells are Mach keys as floats.
/// - **Row 1..N**: first cell is α in **degrees**, remaining cells are Cd
///   values at the corresponding Mach keys.
///
/// α is converted from degrees to radians on the way in so the returned
/// table matches the JSBSim-facing convention that the rest of `core`
/// already assumes.
pub fn load_cd_table_deg(path: &Path) -> Result<Cd0AlphaMachTable> {
    let rows = tokenize(path)?;
    if rows.len() < 2 {
        bail!(
            "2D table {} needs a header row and at least one data row",
            path.display()
        );
    }
    let mut iter = rows.into_iter();
    let header = iter.next().expect("≥2 rows");
    if header.len() < 2 {
        bail!(
            "2D table header in {} needs ≥1 Mach key (got cells {:?})",
            path.display(),
            header
        );
    }
    let mach_keys: Vec<f64> = header[1..]
        .iter()
        .map(|c| {
            c.parse::<f64>().map_err(|_| {
                anyhow!(
                    "Mach key {:?} in header of {} is not a number",
                    c,
                    path.display()
                )
            })
        })
        .collect::<Result<_>>()?;
    let ncols = mach_keys.len();

    let mut rows_out: Vec<Vec<f64>> = Vec::new();
    for row in iter {
        if row.len() != ncols + 1 {
            bail!(
                "2D table row in {} has {} cells, expected {} (1 alpha + {} mach)",
                path.display(),
                row.len(),
                ncols + 1,
                ncols,
            );
        }
        let alpha_deg: f64 = row[0].parse().map_err(|_| {
            anyhow!(
                "alpha cell {:?} in {} is not a number",
                row[0],
                path.display()
            )
        })?;
        let mut out_row = Vec::with_capacity(ncols + 1);
        out_row.push(alpha_deg.to_radians());
        for cell in &row[1..] {
            let v: f64 = cell.parse().map_err(|_| {
                anyhow!("Cd cell {:?} in {} is not a number", cell, path.display())
            })?;
            out_row.push(v);
        }
        rows_out.push(out_row);
    }
    Ok(Cd0AlphaMachTable {
        mach_keys,
        rows: rows_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmpfile(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("simulator_cli_tests");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_1d_with_header() {
        let p = tmpfile(
            "with_header.csv",
            "time,thrust\n0.0,10.0\n1.0,20.0\n",
        );
        let data = load_1d(&p).unwrap();
        assert_eq!(data, vec![[0.0, 10.0], [1.0, 20.0]]);
    }

    #[test]
    fn load_1d_without_header() {
        let p = tmpfile("no_header.csv", "0.0,10.0\n1.0,20.0\n");
        let data = load_1d(&p).unwrap();
        assert_eq!(data, vec![[0.0, 10.0], [1.0, 20.0]]);
    }

    #[test]
    fn load_1d_tab_separated() {
        let p = tmpfile("tabbed.csv", "mach\tcd\n0.5\t0.3\n1.5\t0.6\n");
        let data = load_1d(&p).unwrap();
        assert_eq!(data, vec![[0.5, 0.3], [1.5, 0.6]]);
    }

    #[test]
    fn load_1d_whitespace_separated() {
        // Mirrors `input_reference/tables/cnmach.csv` style: "0\t,0".
        let p = tmpfile("ws.csv", "0    1.0\n0.5  2.0\n1.0  3.0\n");
        let data = load_1d(&p).unwrap();
        assert_eq!(data, vec![[0.0, 1.0], [0.5, 2.0], [1.0, 3.0]]);
    }

    #[test]
    fn load_1d_skips_comment_lines() {
        let p = tmpfile(
            "commented.csv",
            "# leading comment\ntime,thrust\n# interleaved\n0.0,5.0\n1.0,10.0\n",
        );
        let data = load_1d(&p).unwrap();
        assert_eq!(data, vec![[0.0, 5.0], [1.0, 10.0]]);
    }

    #[test]
    fn load_cd_table_deg_converts_to_rad() {
        let p = tmpfile(
            "cd2d.csv",
            "alpha_deg,0.0,1.0\n0.0,0.5,0.8\n90.0,0.6,0.9\n",
        );
        let t = load_cd_table_deg(&p).unwrap();
        assert_eq!(t.mach_keys, vec![0.0, 1.0]);
        assert_eq!(t.rows.len(), 2);
        assert!((t.rows[0][0] - 0.0).abs() < 1e-12, "α=0 rad expected");
        assert!(
            (t.rows[1][0] - std::f64::consts::FRAC_PI_2).abs() < 1e-12,
            "α=90° → π/2 rad"
        );
        // Cd columns preserved as-is
        assert!((t.rows[0][1] - 0.5).abs() < 1e-12);
        assert!((t.rows[1][2] - 0.9).abs() < 1e-12);
    }

    #[test]
    fn load_cd_table_rejects_ragged_rows() {
        let p = tmpfile(
            "ragged.csv",
            "alpha_deg,0.0,1.0\n0.0,0.5,0.8\n10.0,0.6\n",
        );
        let err = load_cd_table_deg(&p).unwrap_err();
        assert!(
            err.to_string().contains("cells"),
            "expected cell-count error, got {err}"
        );
    }

    #[test]
    fn load_cd_table_rejects_header_only_file() {
        let p = tmpfile("hdr_only.csv", "alpha_deg,0.0,1.0\n");
        let err = load_cd_table_deg(&p).unwrap_err();
        assert!(
            err.to_string().contains("at least one data row"),
            "expected data-row error, got {err}"
        );
    }
}
