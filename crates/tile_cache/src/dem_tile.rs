//! DEM tile cache for MapLibre terrain display (Terrarium-encoded PNG).
//!
//! Downloads GSI DEM5A PNG tiles at any requested zoom level, converts
//! the RGB elevation encoding to Terrarium format, and caches the result
//! as PNG bytes in `{cache_dir}/trajec_simu_dem/dem_terrain.mbtiles`.
//!
//! Terrarium encoding: height_m = R*256 + G + B/256 - 32768
//! TMS y-axis convention: tile_row = (2^zoom - 1) - ty.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

const GSI_BASE: &str = "https://cyberjapandata.gsi.go.jp/xyz/dem5a_png";            
const TILE_SIZE: u32 = 256;

type TileKey = (u8, u32, u32); // (zoom, tx, ty)
type TileArc = Arc<Vec<u8>>;   // Terrarium PNG bytes

pub struct DemTileCache {
    db:  Mutex<Connection>,
    mem: RwLock<HashMap<TileKey, TileArc>>,
}

impl DemTileCache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().context("cannot find OS cache dir")?;
        let dir  = base.join("trajec_simu_dem");
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("dem_terrain.mbtiles");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("opening MBTiles {}", db_path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS metadata (name TEXT, value TEXT);
             CREATE TABLE IF NOT EXISTS tiles (
                 zoom_level  INTEGER NOT NULL,
                 tile_column INTEGER NOT NULL,
                 tile_row    INTEGER NOT NULL,
                 tile_data   BLOB    NOT NULL,
                 PRIMARY KEY (zoom_level, tile_column, tile_row)
             );",
        )?;
        Ok(Self {
            db:  Mutex::new(conn),
            mem: RwLock::new(HashMap::new()),
        })
    }

    /// Returns Terrarium-encoded PNG bytes, or None on 404 from GSI.
    /// Three-layer cache: memory → MBTiles DB → GSI network + conversion.
    pub fn get_tile(&self, zoom: u8, tx: u32, ty: u32) -> Result<Option<Arc<Vec<u8>>>> {
        if let Some(arc) = self.mem.read().unwrap().get(&(zoom, tx, ty)) {
            return Ok(Some(arc.clone()));
        }

        let arc = match self.load_tile_db(zoom, tx, ty)? {
            Some(bytes) => Arc::new(bytes),
            None => match self.download_as_terrarium(zoom, tx, ty)? {
                Some(png) => {
                    if let Err(e) = self.save_tile_db(zoom, tx, ty, &png) {
                        log::warn!("dem_tile cache write {zoom}/{tx}/{ty}: {e:#}");
                    }
                    Arc::new(png)
                }
                None => return Ok(None),
            },
        };

        Ok(Some(
            self.mem
                .write()
                .unwrap()
                .entry((zoom, tx, ty))
                .or_insert(arc)
                .clone(),
        ))
    }

    fn tms_row(zoom: u8, ty: u32) -> u32 {
        ((1u64 << zoom) as u32).wrapping_sub(1).wrapping_sub(ty)
    }

    fn load_tile_db(&self, zoom: u8, tx: u32, ty: u32) -> Result<Option<Vec<u8>>> {
        let tms_row = Self::tms_row(zoom, ty);
        let conn = self.db.lock().unwrap();
        let blob: Option<Vec<u8>> = conn
            .query_row(
                "SELECT tile_data FROM tiles \
                 WHERE zoom_level=?1 AND tile_column=?2 AND tile_row=?3",
                params![zoom as i64, tx, tms_row],
                |row| row.get(0),
            )
            .optional()?;
        Ok(blob)
    }

    fn save_tile_db(&self, zoom: u8, tx: u32, ty: u32, data: &[u8]) -> Result<()> {
        let tms_row = Self::tms_row(zoom, ty);
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO tiles (zoom_level, tile_column, tile_row, tile_data) \
             VALUES (?1, ?2, ?3, ?4)",
            params![zoom as i64, tx, tms_row, data],
        )?;
        Ok(())
    }

    fn download_as_terrarium(&self, zoom: u8, tx: u32, ty: u32) -> Result<Option<Vec<u8>>> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build();
        let url = format!("{GSI_BASE}/{zoom}/{tx}/{ty}.png");
        let gsi_png = match agent.get(&url).call() {
            Ok(resp) => {
                let mut bytes = Vec::new();
                resp.into_reader()
                    .read_to_end(&mut bytes)
                    .context("reading DEM tile response")?;
                bytes
            }
            Err(ureq::Error::Status(404, _)) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!("{e}").context(format!("GET {url}"))),
        };

        let terrarium = gsi_png_to_terrarium_png(&gsi_png)
            .with_context(|| format!("converting DEM tile {zoom}/{tx}/{ty}"))?;
        Ok(Some(terrarium))
    }
}

// ── Pixel conversion ─────────────────────────────────────────────────────────

/// Decodes a raw GSI DEM5A RGB PNG and returns a Terrarium-encoded RGB PNG.
fn gsi_png_to_terrarium_png(gsi_bytes: &[u8]) -> Result<Vec<u8>> {
    let decoder = png::Decoder::new(std::io::Cursor::new(gsi_bytes));
    let mut reader = decoder.read_info().context("PNG read_info")?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).context("PNG next_frame")?;

    if info.color_type != png::ColorType::Rgb {
        bail!("expected RGB PNG from GSI, got {:?}", info.color_type);
    }

    let pixels = &buf[..info.buffer_size()];
    let mut terrarium_rgb = vec![0u8; pixels.len()];

    for (i, chunk) in pixels.chunks_exact(3).enumerate() {
        let [r, g, b] = gsi_pixel_to_terrarium(chunk[0], chunk[1], chunk[2]);
        terrarium_rgb[i * 3]     = r;
        terrarium_rgb[i * 3 + 1] = g;
        terrarium_rgb[i * 3 + 2] = b;
    }

    encode_rgb_png(&terrarium_rgb, TILE_SIZE, TILE_SIZE)
}

/// Converts a single GSI DEM5A pixel to Terrarium RGB.
/// No-data pixels (raw == 0x800000) map to sea level (0m).
fn gsi_pixel_to_terrarium(r: u8, g: u8, b: u8) -> [u8; 3] {
    let raw = (r as u32) * 65536 + (g as u32) * 256 + b as u32;
    let h = if raw == 8_388_608 {
        0.0f64  // no-data → sea level
    } else if raw < 8_388_608 {
        raw as f64 * 0.01
    } else {
        (raw as i64 - 16_777_216) as f64 * 0.01
    };
    elevation_to_terrarium(h)
}

/// Encodes elevation in metres to Terrarium RGB bytes.
/// Terrarium: height = R*256 + G + B/256 - 32768
fn elevation_to_terrarium(h: f64) -> [u8; 3] {
    // clamp to [-32768, 32767.999] to stay in u8 range
    let shifted = (h + 32768.0).clamp(0.0, 65535.999_999);
    let r = (shifted / 256.0).floor() as u8;
    let g = (shifted % 256.0).floor() as u8;
    let b = (shifted.fract() * 256.0).round().min(255.0) as u8;
    [r, g, b]
}

fn encode_rgb_png(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().context("PNG write_header")?;
    writer.write_image_data(rgb).context("PNG write_image_data")?;
    drop(writer);
    Ok(out)
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrarium_sea_level() {
        let [r, g, b] = elevation_to_terrarium(0.0);
        let decoded = r as f64 * 256.0 + g as f64 + b as f64 / 256.0 - 32768.0;
        assert!(decoded.abs() < 0.01, "decoded={decoded}");
    }

    #[test]
    fn terrarium_positive() {
        let h = 3776.0; // Mt. Fuji approx
        let [r, g, b] = elevation_to_terrarium(h);
        let decoded = r as f64 * 256.0 + g as f64 + b as f64 / 256.0 - 32768.0;
        assert!((decoded - h).abs() < 0.01, "decoded={decoded} expected={h}");
    }

    #[test]
    fn terrarium_negative() {
        let h = -50.0;
        let [r, g, b] = elevation_to_terrarium(h);
        let decoded = r as f64 * 256.0 + g as f64 + b as f64 / 256.0 - 32768.0;
        assert!((decoded - h).abs() < 0.01, "decoded={decoded} expected={h}");
    }

    #[test]
    fn gsi_nodata_to_sealevel() {
        // raw = 0x800000 → no-data → sea level
        let [r, g, b] = gsi_pixel_to_terrarium(0x80, 0x00, 0x00);
        let decoded = r as f64 * 256.0 + g as f64 + b as f64 / 256.0 - 32768.0;
        assert!(decoded.abs() < 0.01, "no-data should map to 0m, got {decoded}");
    }
}