//! GSI DEM tile cache — elevation lookup and MapLibre terrain display.
//!
//! Two structs share download and pixel-decoding logic:
//!
//! * [`DemCache`] — zoom-15 tiles decoded to f32 grids, gzip-compressed in
//!   `dem.mbtiles`. Used by the CLI for elevation queries.
//! * [`DemTileCache`] — tiles at any zoom level, re-encoded as Terrarium PNG,
//!   stored in `dem_terrain.mbtiles`. Served by the Tauri GUI for MapLibre terrain.
//!
//! MBTiles schema (both DBs):
//!   `tiles (zoom_level, tile_column, tile_row PRIMARY KEY, tile_data BLOB)`
//!
//! TMS y-axis convention: `tile_row = (2^zoom - 1) - ty`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

// ── Constants ────────────────────────────────────────────────────────────────

pub const ZOOM: u32 = 15;
const TILE_PIXELS: usize = 256;
const GRID_SIZE: usize = TILE_PIXELS * TILE_PIXELS; // 65536
const NO_DATA: f32 = f32::NAN;

type TileGrid = Box<[f32; GRID_SIZE]>;

// ── Coordinate math (zoom-15) ─────────────────────────────────────────────────

/// Returns `(tile_x, tile_y)` for a lat/lon at zoom `ZOOM` (15).
pub fn lat_lon_to_tile(lat_deg: f64, lon_deg: f64) -> (u32, u32) {
    let n = (1u64 << ZOOM) as f64;
    let tx = ((lon_deg + 180.0) / 360.0 * n).floor() as u32;
    let lat_r = lat_deg.to_radians();
    let ty = ((1.0 - (lat_r.tan() + 1.0 / lat_r.cos()).ln() / std::f64::consts::PI) / 2.0 * n)
        .floor() as u32;
    (tx, ty)
}

/// Returns `(pixel_x, pixel_y)` within the tile for the given lat/lon.
pub fn lat_lon_to_pixel(lat_deg: f64, lon_deg: f64) -> (usize, usize) {
    let n = (1u64 << ZOOM) as f64;
    let fx = (lon_deg + 180.0) / 360.0 * n;
    let lat_r = lat_deg.to_radians();
    let fy = (1.0 - (lat_r.tan() + 1.0 / lat_r.cos()).ln() / std::f64::consts::PI) / 2.0 * n;

    let (tx, ty) = lat_lon_to_tile(lat_deg, lon_deg);
    let px = ((fx - tx as f64) * TILE_PIXELS as f64).floor() as usize;
    let py = ((fy - ty as f64) * TILE_PIXELS as f64).floor() as usize;
    (px.min(TILE_PIXELS - 1), py.min(TILE_PIXELS - 1))
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn tms_row(zoom: u32, ty: u32) -> u32 {
    ((1u64 << zoom) as u32).wrapping_sub(1).wrapping_sub(ty)
}

fn decode_elevation(r: u8, g: u8, b: u8) -> f32 {
    let raw = (r as u32) * 65536 + (g as u32) * 256 + b as u32;
    if raw == 8_388_608 {
        NO_DATA
    } else if raw < 8_388_608 {
        raw as f32 * 0.01
    } else {
        (raw as i32 - 16_777_216) as f32 * 0.01
    }
}

/// Downloads a raw GSI DEM5A PNG tile. Returns `None` on 404.
fn download_gsi_png(zoom: u32, tx: u32, ty: u32) -> Result<Option<Vec<u8>>> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(30))
        .build();
    let url = format!("https://cyberjapandata.gsi.go.jp/xyz/dem5a_png/{zoom}/{tx}/{ty}.png");
    match agent.get(&url).call() {
        Ok(resp) => {
            let mut bytes = Vec::new();
            resp.into_reader()
                .read_to_end(&mut bytes)
                .context("reading DEM tile")?;
            Ok(Some(bytes))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("{e}").context(format!("GET {url}"))),
    }
}

/// Decodes a GSI DEM5A RGB PNG into a 256×256 f32 elevation grid.
fn decode_dem_png(bytes: &[u8]) -> Result<TileGrid> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().context("PNG read_info failed")?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .context("PNG next_frame failed")?;

    if info.color_type != png::ColorType::Rgb {
        bail!("expected RGB PNG, got {:?}", info.color_type);
    }
    if info.width != TILE_PIXELS as u32 || info.height != TILE_PIXELS as u32 {
        bail!(
            "expected {}×{} tile, got {}×{}",
            TILE_PIXELS,
            TILE_PIXELS,
            info.width,
            info.height
        );
    }

    let pixels = &buf[..info.buffer_size()];
    let mut grid = Box::new([0f32; GRID_SIZE]);
    for (i, chunk) in pixels.chunks_exact(3).enumerate() {
        grid[i] = decode_elevation(chunk[0], chunk[1], chunk[2]);
    }
    Ok(grid)
}

// ── DemCache — elevation queries at zoom 15 ───────────────────────────────────

fn encode_grid_gz(grid: &[f32; GRID_SIZE]) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(GRID_SIZE * 2);
    let mut gz = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
    // SAFETY: [f32; N] → bytes is always valid (no padding, no undef bits for f32)
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(grid.as_ptr() as *const u8, GRID_SIZE * 4) };
    gz.write_all(bytes)?;
    gz.finish()?;
    Ok(buf)
}

fn decode_grid_gz(blob: Vec<u8>) -> Result<TileGrid> {
    let mut gz = flate2::read::GzDecoder::new(std::io::Cursor::new(blob));
    let mut bytes = vec![0u8; GRID_SIZE * 4];
    gz.read_exact(&mut bytes)
        .context("reading gzip-compressed tile blob")?;
    let mut grid = Box::new([0f32; GRID_SIZE]);
    // SAFETY: we wrote exactly GRID_SIZE * 4 bytes in encode_grid_gz
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), grid.as_mut_ptr() as *mut u8, GRID_SIZE * 4);
    }
    Ok(grid)
}

/// Three-layer cache (memory → MBTiles → GSI network) for zoom-15 DEM tiles
/// stored as gzip-compressed f32 elevation grids. Shared with the CLI.
pub struct DemCache {
    db: Mutex<Connection>,
    mem: RwLock<HashMap<(u32, u32), Arc<TileGrid>>>,
}

impl DemCache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().context("could not determine OS cache directory")?;
        let dir = base.join("trajec_simu_dem");
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("dem.mbtiles");
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
            db: Mutex::new(conn),
            mem: RwLock::new(HashMap::new()),
        })
    }

    fn load_tile_db(&self, tx: u32, ty: u32) -> Result<Option<TileGrid>> {
        let row = tms_row(ZOOM, ty);
        let conn = self.db.lock().unwrap();
        let blob: Option<Vec<u8>> = conn
            .query_row(
                "SELECT tile_data FROM tiles \
                 WHERE zoom_level=?1 AND tile_column=?2 AND tile_row=?3",
                params![ZOOM, tx, row],
                |r| r.get(0),
            )
            .optional()?;
        match blob {
            None => Ok(None),
            Some(b) => decode_grid_gz(b).map(Some),
        }
    }

    fn save_tile_db(&self, tx: u32, ty: u32, grid: &[f32; GRID_SIZE]) -> Result<()> {
        let row = tms_row(ZOOM, ty);
        let buf = encode_grid_gz(grid)?;
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO tiles \
             (zoom_level, tile_column, tile_row, tile_data) VALUES (?1, ?2, ?3, ?4)",
            params![ZOOM, tx, row, buf],
        )?;
        Ok(())
    }

    fn load_tile(&self, tx: u32, ty: u32) -> Result<Arc<TileGrid>> {
        if let Some(arc) = self.mem.read().unwrap().get(&(tx, ty)) {
            return Ok(arc.clone());
        }
        let arc = match self.load_tile_db(tx, ty)? {
            Some(g) => Arc::new(g),
            None => self.fetch_or_zero(tx, ty),
        };
        Ok(self
            .mem
            .write()
            .unwrap()
            .entry((tx, ty))
            .or_insert(arc)
            .clone())
    }

    fn fetch_or_zero(&self, tx: u32, ty: u32) -> Arc<TileGrid> {
        match download_gsi_png(ZOOM, tx, ty) {
            Ok(Some(bytes)) => match decode_dem_png(&bytes) {
                Ok(g) => {
                    if let Err(e) = self.save_tile_db(tx, ty, &g) {
                        eprintln!("warn: DEM cache write {tx},{ty}: {e:#}");
                    }
                    Arc::new(g)
                }
                Err(e) => {
                    eprintln!("warn: DEM decode {tx},{ty}: {e:#} — using 0 m");
                    Arc::new(Box::new([0f32; GRID_SIZE]))
                }
            },
            Ok(None) => {
                eprintln!("DEM: tile {tx},{ty} not found on server — storing as 0 m");
                let g = Box::new([0f32; GRID_SIZE]);
                if let Err(e) = self.save_tile_db(tx, ty, &g) {
                    eprintln!("warn: DEM cache write {tx},{ty}: {e:#}");
                }
                Arc::new(g)
            }
            Err(e) => {
                eprintln!("warn: DEM download {tx},{ty}: {e:#} — using 0 m");
                Arc::new(Box::new([0f32; GRID_SIZE]))
            }
        }
    }

    /// Terrain elevation in metres ASL at the given position.
    /// Returns `None` if the tile pixel carries no data or the tile is unavailable.
    pub fn get_elevation(&self, lat_deg: f64, lon_deg: f64) -> Result<Option<f64>> {
        let (tx, ty) = lat_lon_to_tile(lat_deg, lon_deg);
        let grid = match self.load_tile(tx, ty) {
            Ok(g) => g,
            Err(e) => {
                log::warn!("DEM tile {tx},{ty} unavailable: {e:#}");
                return Ok(None);
            }
        };
        let (px, py) = lat_lon_to_pixel(lat_deg, lon_deg);
        let val = grid[py * TILE_PIXELS + px];
        if val.is_nan() {
            Ok(None)
        } else {
            Ok(Some(val as f64))
        }
    }
}

// ── DemTileCache — Terrarium PNG tiles for MapLibre terrain ──────────────────

/// Encodes elevation in metres to Terrarium RGB bytes.
/// Terrarium: `height = R×256 + G + B/256 − 32768`
fn elevation_to_terrarium(h: f64) -> [u8; 3] {
    let shifted = (h + 32768.0).clamp(0.0, 65_535.999_999);
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
    writer
        .write_image_data(rgb)
        .context("PNG write_image_data")?;
    drop(writer);
    Ok(out)
}

/// Converts a GSI DEM5A PNG into a Terrarium-encoded RGB PNG.
/// No-data pixels map to sea level (0 m).
fn gsi_png_to_terrarium_png(gsi_bytes: &[u8]) -> Result<Vec<u8>> {
    let grid = decode_dem_png(gsi_bytes)?;
    let mut rgb = vec![0u8; GRID_SIZE * 3];
    for (i, &h) in grid.iter().enumerate() {
        let [r, g, b] = elevation_to_terrarium(if h.is_nan() { 0.0 } else { h as f64 });
        rgb[i * 3] = r;
        rgb[i * 3 + 1] = g;
        rgb[i * 3 + 2] = b;
    }
    encode_rgb_png(&rgb, TILE_PIXELS as u32, TILE_PIXELS as u32)
}

/// Three-layer cache (memory → MBTiles → GSI network) for DEM tiles at any
/// zoom level, stored as Terrarium-encoded PNG for MapLibre `raster-dem`.
pub struct DemTileCache {
    db: Mutex<Connection>,
    mem: RwLock<HashMap<(u8, u32, u32), Arc<Vec<u8>>>>,
}

impl DemTileCache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().context("cannot find OS cache dir")?;
        let dir = base.join("trajec_simu_dem");
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
            db: Mutex::new(conn),
            mem: RwLock::new(HashMap::new()),
        })
    }

    /// Returns Terrarium-encoded PNG bytes, or `None` on 404 from GSI.
    pub fn get_tile(&self, zoom: u8, tx: u32, ty: u32) -> Result<Option<Arc<Vec<u8>>>> {
        if let Some(arc) = self.mem.read().unwrap().get(&(zoom, tx, ty)) {
            return Ok(Some(arc.clone()));
        }
        let arc = match self.load_tile_db(zoom, tx, ty)? {
            Some(bytes) => Arc::new(bytes),
            None => match download_gsi_png(zoom as u32, tx, ty)? {
                Some(raw_png) => {
                    let png = gsi_png_to_terrarium_png(&raw_png)
                        .with_context(|| format!("converting DEM tile {zoom}/{tx}/{ty}"))?;
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

    fn load_tile_db(&self, zoom: u8, tx: u32, ty: u32) -> Result<Option<Vec<u8>>> {
        let row = tms_row(zoom as u32, ty);
        let conn = self.db.lock().unwrap();
        let blob: Option<Vec<u8>> = conn
            .query_row(
                "SELECT tile_data FROM tiles \
                 WHERE zoom_level=?1 AND tile_column=?2 AND tile_row=?3",
                params![zoom as i64, tx, row],
                |r| r.get(0),
            )
            .optional()?;
        Ok(blob)
    }

    fn save_tile_db(&self, zoom: u8, tx: u32, ty: u32, data: &[u8]) -> Result<()> {
        let row = tms_row(zoom as u32, ty);
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO tiles \
             (zoom_level, tile_column, tile_row, tile_data) VALUES (?1, ?2, ?3, ?4)",
            params![zoom as i64, tx, row, data],
        )?;
        Ok(())
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- DemCache (coordinate math, elevation decode, gz roundtrip) ---

    #[test]
    fn tile_coords_japan() {
        let (tx, ty) = lat_lon_to_tile(40.247, 140.010);
        assert!((29100..29160).contains(&tx), "tx={tx}");
        assert!((12350..12410).contains(&ty), "ty={ty}");
    }

    #[test]
    fn pixel_within_tile_bounds() {
        let (px, py) = lat_lon_to_pixel(40.247, 140.010);
        assert!(px < TILE_PIXELS, "px={px}");
        assert!(py < TILE_PIXELS, "py={py}");
    }

    #[test]
    fn decode_elevation_positive() {
        let raw: u32 = 100_000;
        let r = (raw >> 16) as u8;
        let g = ((raw >> 8) & 0xFF) as u8;
        let b = (raw & 0xFF) as u8;
        let h = decode_elevation(r, g, b);
        assert!((h - 1000.0).abs() < 0.01, "h={h}");
    }

    #[test]
    fn decode_elevation_no_data() {
        let h = decode_elevation(0x80, 0x00, 0x00);
        assert!(h.is_nan(), "expected NaN for no-data pixel");
    }

    #[test]
    fn decode_elevation_negative() {
        let raw: u32 = 16_777_116;
        let r = (raw >> 16) as u8;
        let g = ((raw >> 8) & 0xFF) as u8;
        let b = (raw & 0xFF) as u8;
        let h = decode_elevation(r, g, b);
        assert!((h - (-1.0)).abs() < 0.01, "h={h}");
    }

    #[test]
    fn gz_roundtrip() {
        let mut grid = Box::new([0f32; GRID_SIZE]);
        grid[0] = 123.45;
        grid[GRID_SIZE - 1] = -9.99;
        let blob = encode_grid_gz(&grid).unwrap();
        let decoded = decode_grid_gz(blob).unwrap();
        assert!((decoded[0] - 123.45).abs() < 1e-4);
        assert!((decoded[GRID_SIZE - 1] - (-9.99)).abs() < 1e-4);
    }

    #[test]
    fn tms_row_zoom15() {
        // zoom=15, ty=12375 → tms_row = 32767 - 12375 = 20392
        assert_eq!(tms_row(15, 12375), 20392);
    }

    // --- DemTileCache (Terrarium encode/decode) ---

    #[test]
    fn terrarium_sea_level() {
        let [r, g, b] = elevation_to_terrarium(0.0);
        let decoded = r as f64 * 256.0 + g as f64 + b as f64 / 256.0 - 32768.0;
        assert!(decoded.abs() < 0.01, "decoded={decoded}");
    }

    #[test]
    fn terrarium_positive() {
        let h = 3776.0; // 富士山近似
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
        // raw = 0x800000 → no-data → sea level (0 m)
        let h = decode_elevation(0x80, 0x00, 0x00);
        assert!(h.is_nan());
        // gsi_png_to_terrarium_png maps NaN → 0 m
        let [r, g, b] = elevation_to_terrarium(0.0);
        let decoded = r as f64 * 256.0 + g as f64 + b as f64 / 256.0 - 32768.0;
        assert!(
            decoded.abs() < 0.01,
            "no-data should map to 0 m, got {decoded}"
        );
    }
}
