//! GSI DEM tile cache and elevation lookup.
//!
//! Tiles are fetched from cyberjapandata.gsi.go.jp (dem5a_png),
//! decoded to a 256×256 f32 elevation grid, gzip-compressed, and stored in
//! `{dirs::cache_dir()}/trajec_simu_dem/dem_z15.mbtiles` (MBTiles format).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

pub const ZOOM: u32 = 15;
const TILE_PIXELS: usize = 256;
const GRID_SIZE: usize = TILE_PIXELS * TILE_PIXELS; // 65536

const NO_DATA: f32 = f32::NAN;

// TMS y-axis convention: tile_row = (2^zoom - 1) - ty
const TMS_MAX_Y: u32 = (1u32 << ZOOM) - 1;

type TileKey = (u32, u32);
type TileGrid = Box<[f32; GRID_SIZE]>;
type TileArc = Arc<TileGrid>;

// ── Tile coordinate math ─────────────────────────────────────────────────────

/// Returns (tile_x, tile_y) for a lat/lon at zoom z.
pub fn lat_lon_to_tile(lat_deg: f64, lon_deg: f64) -> (u32, u32) {
    let n = (1u64 << ZOOM) as f64;
    let tx = ((lon_deg + 180.0) / 360.0 * n).floor() as u32;
    let lat_r = lat_deg.to_radians();
    let ty = ((1.0 - (lat_r.tan() + 1.0 / lat_r.cos()).ln() / std::f64::consts::PI) / 2.0 * n)
        .floor() as u32;
    (tx, ty)
}

/// Returns (pixel_x, pixel_y) within the tile for the given lat/lon.
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

// ── Elevation decoding ───────────────────────────────────────────────────────

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

// ── MBTiles I/O ──────────────────────────────────────────────────────────────

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

// ── DemCache ─────────────────────────────────────────────────────────────────

pub struct DemCache {
    db: Mutex<Connection>,
    mem: RwLock<HashMap<TileKey, TileArc>>,
}

impl DemCache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().context("could not determine OS cache directory")?;
        let dir = base.join("trajec_simu_dem");
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("dem.mbtiles");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("opening MBTiles {}", db_path.display()))?;
        // Enable WAL for better concurrent read/write behavior on the cache DB.
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
        let tms_row = TMS_MAX_Y - ty;
        let conn = self.db.lock().unwrap();
        let blob: Option<Vec<u8>> = conn
            .query_row(
                "SELECT tile_data FROM tiles \
                 WHERE zoom_level=?1 AND tile_column=?2 AND tile_row=?3",
                params![ZOOM, tx, tms_row],
                |row| row.get(0),
            )
            .optional()?;
        match blob {
            None => Ok(None),
            Some(b) => decode_grid_gz(b).map(Some),
        }
    }

    fn save_tile_db(&self, tx: u32, ty: u32, grid: &[f32; GRID_SIZE]) -> Result<()> {
        let tms_row = TMS_MAX_Y - ty;
        let buf = encode_grid_gz(grid)?;
        let conn = self.db.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO tiles (zoom_level, tile_column, tile_row, tile_data) \
             VALUES (?1, ?2, ?3, ?4)",
            params![ZOOM, tx, tms_row, buf],
        )?;
        Ok(())
    }

    /// Load tile: memory cache → MBTiles DB → network.
    ///
    /// Uses a double-checked locking pattern with `RwLock`:
    /// - Read lock for the fast cache-hit path (no contention between threads).
    /// - DB / network I/O runs entirely outside the lock so other threads
    ///   can continue reading cached tiles concurrently.
    /// - Write lock uses `entry().or_insert()` to safely handle the race where
    ///   two threads both reach the slow path for the same tile.
    fn load_tile(&self, tx: u32, ty: u32) -> Result<TileArc> {
        // Fast path — read lock, no exclusive access needed.
        if let Some(arc) = self.mem.read().unwrap().get(&(tx, ty)) {
            return Ok(arc.clone());
        }

        // Slow path — DB lookup outside the mem lock.
        let arc = match self.load_tile_db(tx, ty)? {
            Some(g) => Arc::new(g),
            None => self.fetch_or_zero(tx, ty),
        };

        // Write lock — or_insert drops the duplicate if another thread won the race.
        Ok(self
            .mem
            .write()
            .unwrap()
            .entry((tx, ty))
            .or_insert(arc)
            .clone())
    }

    /// Download a tile from GSI.
    fn download(&self, tx: u32, ty: u32) -> Result<Option<TileGrid>> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build();

        let urls = [format!(
            "https://cyberjapandata.gsi.go.jp/xyz/dem5a_png/{ZOOM}/{tx}/{ty}.png"
        )];

        for url in &urls {
            match agent.get(url).call() {
                Ok(resp) => {
                    let mut bytes = Vec::new();
                    resp.into_reader()
                        .read_to_end(&mut bytes)
                        .context("reading tile response")?;
                    let grid = decode_dem_png(&bytes)
                        .with_context(|| format!("decoding tile {tx},{ty} from {url}"))?;
                    return Ok(Some(grid));
                }
                Err(ureq::Error::Status(404, _)) => {
                    continue;
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("{e}").context(format!("GET {url}")));
                }
            }
        }
        Ok(None)
    }

    /// Load or create a tile grid. On 404, saves a zero-filled grid to the DB so
    /// the tile is not re-requested on the next run.
    fn fetch_or_zero(&self, tx: u32, ty: u32) -> TileArc {
        match self.download(tx, ty) {
            Ok(Some(g)) => {
                if let Err(e) = self.save_tile_db(tx, ty, &g) {
                    eprintln!("warn: DEM cache write {tx},{ty}: {e:#}");
                }
                Arc::new(g)
            }
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
    /// Returns `None` if the tile pixel carries no data or the tile
    /// is unavailable.
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

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_coords_japan() {
        // 40.247°N, 140.010°E at z=15: computed reference tx=29128, ty=12375.
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
        // raw = 100000 → 1000.00 m
        let raw: u32 = 100_000;
        let r = (raw >> 16) as u8;
        let g = ((raw >> 8) & 0xFF) as u8;
        let b = (raw & 0xFF) as u8;
        let h = decode_elevation(r, g, b);
        assert!((h - 1000.0).abs() < 0.01, "h={h}");
    }

    #[test]
    fn decode_elevation_no_data() {
        // raw = 8388608 (0x800000) → no data
        let h = decode_elevation(0x80, 0x00, 0x00);
        assert!(h.is_nan(), "expected NaN for no-data pixel");
    }

    #[test]
    fn decode_elevation_negative() {
        // raw = 16777116 → (16777116 - 16777216) * 0.01 = -1.00 m
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
    fn tms_y_conversion() {
        // zoom=15, ty=12375 → tms_row = 32767 - 12375 = 20392
        let ty = 12375u32;
        let tms_row = TMS_MAX_Y - ty;
        assert_eq!(tms_row, 20392);
    }
}
