//! Summary output for the landing-area sweep.
//!
//! Produces two files in the root output directory:
//!   - `landing_summary.csv`: per-condition wind → landing position
//!   - `landing_range.kml`:   convex hull per wind speed for ballistic and
//!     parachute landings, with individual point placemarks

use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};

/// Landing positions extracted from a single wind condition.
#[derive(Debug, Clone)]
pub struct ConditionResult {
    pub speed_mps: f64,
    pub dir_deg: f64,
    pub landed: Option<(f64, f64)>,           // (lat_deg, lon_deg)
    pub parachute_landed: Option<(f64, f64)>, // (lat_deg, lon_deg)
}

// ── CSV ───────────────────────────────────────────────────────────────────────

/// Write `landing_summary.csv` under `out_dir`.
pub fn write_summary_csv(out_dir: &Path, results: &[ConditionResult]) -> Result<()> {
    let path = out_dir.join("landing_summary.csv");
    let f = fs::File::create(&path).with_context(|| format!("creating {}", path.display()))?;
    let mut w = BufWriter::new(f);
    writeln!(
        w,
        "speed_mps,dir_deg,ballistic_lat,ballistic_lon,parachute_lat,parachute_lon"
    )?;
    for r in results {
        let (blat, blon) = r.landed.unwrap_or((f64::NAN, f64::NAN));
        let (plat, plon) = r.parachute_landed.unwrap_or((f64::NAN, f64::NAN));
        writeln!(
            w,
            "{:.4},{:.4},{:.9},{:.9},{:.9},{:.9}",
            r.speed_mps, r.dir_deg, blat, blon, plat, plon,
        )?;
    }
    w.flush()?;
    Ok(())
}

// ── KML ───────────────────────────────────────────────────────────────────────

// KML colors are ABGR hex.
// ff00ffff: opaque yellow  (ballistic)
// ff0000ff: opaque red     (parachute)
// 3300ffff: 20% yellow     (ballistic hull fill)
// 330000ff: 20% red        (parachute hull fill)

const KML_HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
<Document>
  <name>Landing area</name>
  <Style id="ballistic_hull">
    <LineStyle><color>ff005ed5</color><width>2</width></LineStyle>
    <PolyStyle><color>4D005ED5</color></PolyStyle>
  </Style>
  <Style id="parachute_hull">
    <LineStyle><color>ffa779cc</color><width>2</width></LineStyle>
    <PolyStyle><color>4Da779cc</color></PolyStyle>
  </Style>
  <Style id="ballistic_pt">
    <IconStyle><color>ff005ed5</color><scale>0.7</scale>
      <Icon><href>http://maps.google.com/mapfiles/kml/shapes/placemark_circle.png</href></Icon>
    </IconStyle>
  </Style>
  <Style id="parachute_pt">
    <IconStyle><color>ffa779cc</color><scale>0.7</scale>
      <Icon><href>http://maps.google.com/mapfiles/kml/shapes/placemark_circle.png</href></Icon>
    </IconStyle>
  </Style>
"#;

/// Write `landing_range.kml` under `out_dir`.
///
/// One convex-hull polygon per unique wind speed, separate folders for
/// ballistic (`Landed`) and parachute (`ParachuteLanded`).
pub fn write_range_kml(out_dir: &Path, results: &[ConditionResult]) -> Result<()> {
    let path = out_dir.join("landing_range.kml");
    let f = fs::File::create(&path).with_context(|| format!("creating {}", path.display()))?;
    let mut w = BufWriter::new(f);

    w.write_all(KML_HEADER.as_bytes())?;
    write_landing_folder(
        &mut w,
        "Parachute landing",
        "parachute_hull",
        "parachute_pt",
        results,
        |r| r.parachute_landed,
    )?;
    write_landing_folder(
        &mut w,
        "Ballistic landing",
        "ballistic_hull",
        "ballistic_pt",
        results,
        |r| r.landed,
    )?;


    writeln!(w, "</Document>\n</kml>")?;
    w.flush()?;
    Ok(())
}

fn write_landing_folder<F>(
    w: &mut impl Write,
    folder_name: &str,
    hull_style: &str,
    pt_style: &str,
    results: &[ConditionResult],
    get_pt: F,
) -> Result<()>
where
    F: Fn(&ConditionResult) -> Option<(f64, f64)>,
{
    writeln!(w, "  <Folder>\n    <name>{folder_name}</name>")?;

    // Collect unique speeds (sorted ascending).
    let mut speeds: Vec<f64> = results.iter().map(|r| r.speed_mps).collect();
    speeds.sort_by(f64::total_cmp);
    speeds.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    // Convex hull polygon per wind speed.
    for &spd in &speeds {
        let pts: Vec<(f64, f64)> = results
            .iter()
            .filter(|r| (r.speed_mps - spd).abs() < 1e-9)
            .filter_map(&get_pt)
            .collect();

        let hull = convex_hull(pts);
        // Need at least 4 coordinates to form a closed polygon (3 unique + 1 closing).
        if hull.len() < 4 {
            continue;
        }

        writeln!(
            w,
            "    <Placemark>\n      <name>{folder_name} {spd:.1} m/s</name>\n      \
             <styleUrl>#{hull_style}</styleUrl>\n      <Polygon>\n        \
             <outerBoundaryIs><LinearRing>\n          \
             <altitudeMode>clampToGround</altitudeMode>\n          <coordinates>"
        )?;
        for (lat, lon) in &hull {
            writeln!(w, "            {lon:.7},{lat:.7},0")?;
        }
        writeln!(
            w,
            "          </coordinates>\n        </LinearRing></outerBoundaryIs>\n      \
             </Polygon>\n    </Placemark>"
        )?;
    }

    // Individual point placemarks for every condition.
    writeln!(
        w,
        "    <Folder>\n      <name>{folder_name} points</name>\n      <visibility>0</visibility>"
    )?;
    for r in results {
        if let Some((lat, lon)) = get_pt(r) {
            writeln!(
                w,
                "      <Placemark>\n        <name>{:03.0}</name>\n        \
                 <styleUrl>#{pt_style}</styleUrl>\n        <Point>\n          \
                 <altitudeMode>clampToGround</altitudeMode>\n          \
                 <coordinates>{:.7},{:.7},0</coordinates>\n        </Point>\n      </Placemark>",
                r.dir_deg, lon, lat,
            )?;
        }
    }
    writeln!(w, "    </Folder>")?;

    writeln!(w, "  </Folder>")?;
    Ok(())
}

// ── Convex hull (Andrew's monotone chain) ────────────────────────────────────

/// Returns the convex hull of `points` sorted by (lon, lat), closed ring
/// (first == last). Uses Andrew's monotone chain algorithm, O(n log n).
/// Returns an empty vec for empty input.
fn convex_hull(mut points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if points.is_empty() {
        return points;
    }

    // Sort by (lon, lat) — x-primary, y-secondary in geographic terms.
    points.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    // Remove exact duplicates.
    points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12);

    let n = points.len();
    if n < 3 {
        if n >= 2 {
            points.push(points[0]);
        }
        return points;
    }

    // 2D cross product (lon=x, lat=y): positive → CCW turn.
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| -> f64 {
        (a.1 - o.1) * (b.0 - o.0) - (a.0 - o.0) * (b.1 - o.1)
    };

    let build_half = |iter: &mut dyn Iterator<Item = (f64, f64)>| -> Vec<(f64, f64)> {
        let mut hull: Vec<(f64, f64)> = Vec::with_capacity(n);
        for p in iter {
            while hull.len() >= 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull
    };

    let mut lower = build_half(&mut points.iter().copied());
    let mut upper = build_half(&mut points.iter().copied().rev());

    // Remove the endpoints shared between the two halves.
    lower.pop();
    upper.pop();
    lower.extend_from_slice(&upper);

    // Close the ring.
    if !lower.is_empty() {
        lower.push(lower[0]);
    }
    lower
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_square() {
        // Four corners of a unit square.
        let pts = vec![
            (0.0_f64, 0.0_f64),
            (0.0, 1.0),
            (1.0, 0.0),
            (1.0, 1.0),
        ];
        let hull = convex_hull(pts);
        // Hull should close (first == last) and have 5 points (4 corners + 1 closing).
        assert_eq!(hull.first(), hull.last(), "hull must be closed");
        assert_eq!(hull.len(), 5, "square hull len={}", hull.len());
    }

    #[test]
    fn hull_collinear_points() {
        // All points on a line — hull degenerates to 2 unique + 1 closing.
        let pts: Vec<(f64, f64)> = (0..5).map(|i| (0.0_f64, i as f64)).collect();
        let hull = convex_hull(pts);
        assert!(hull.len() >= 2, "need at least 2 points for degenerate hull");
    }

    #[test]
    fn hull_empty() {
        let hull = convex_hull(vec![]);
        assert!(hull.is_empty());
    }

    #[test]
    fn hull_single() {
        let hull = convex_hull(vec![(1.0, 2.0)]);
        assert_eq!(hull.len(), 1);
    }
}

