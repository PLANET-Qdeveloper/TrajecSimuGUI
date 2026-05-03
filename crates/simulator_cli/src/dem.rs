//! GSI DEM tile cache and elevation lookup.
//!
//! Tiles are fetched from cyberjapandata.gsi.go.jp (dem5a_png, fallback dem_png),
//! decoded to a 256×256 f32 elevation grid, gzip-compressed, and stored under
//! `{dirs::cache_dir()}/trajec_simu_dem/15/{tx}_{ty}.gz`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{bail, Context, Result};

pub const ZOOM: u32 = 15;
const TILE_PIXELS: usize = 256;
const GRID_SIZE: usize = TILE_PIXELS * TILE_PIXELS; // 65536

// No-data sentinel in the decoded grid.
const NO_DATA: f32 = f32::NAN;

// Cache type aliases for readability.
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
    (
        px.min(TILE_PIXELS - 1),
        py.min(TILE_PIXELS - 1),
    )
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
    let info = reader.next_frame(&mut buf).context("PNG next_frame failed")?;

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

// ── Cache I/O ────────────────────────────────────────────────────────────────

fn save_tile_gz(path: &Path, grid: &[f32; GRID_SIZE]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let f = std::fs::File::create(path)
        .with_context(|| format!("creating cache file {}", path.display()))?;
    let mut gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
    // SAFETY: [f32; N] → bytes is always valid (no padding, no undef bits for f32)
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(grid.as_ptr() as *const u8, GRID_SIZE * 4)
    };
    gz.write_all(bytes)?;
    gz.finish()?;
    Ok(())
}

fn load_tile_gz(path: &Path) -> Result<TileGrid> {
    let f = std::fs::File::open(path)
        .with_context(|| format!("opening cache file {}", path.display()))?;
    let mut gz = flate2::read::GzDecoder::new(f);
    let mut bytes = vec![0u8; GRID_SIZE * 4];
    gz.read_exact(&mut bytes)
        .context("reading gzip-compressed tile")?;
    let mut grid = Box::new([0f32; GRID_SIZE]);
    // SAFETY: we wrote exactly GRID_SIZE * 4 bytes in save_tile_gz
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), grid.as_mut_ptr() as *mut u8, GRID_SIZE * 4);
    }
    Ok(grid)
}

// ── DemCache ─────────────────────────────────────────────────────────────────

pub struct DemCache {
    cache_dir: PathBuf,
    mem: RwLock<HashMap<TileKey, TileArc>>,
}

impl DemCache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir()
            .context("could not determine OS cache directory")?;
        let cache_dir = base.join("trajec_simu_dem").join(ZOOM.to_string());
        Ok(Self {
            cache_dir,
            mem: RwLock::new(HashMap::new()),
        })
    }

    fn tile_path(&self, tx: u32, ty: u32) -> PathBuf {
        self.cache_dir.join(format!("{tx}_{ty}.gz"))
    }

    /// Load tile: memory cache → disk cache → network.
    ///
    /// Uses a double-checked locking pattern with `RwLock`:
    /// - Read lock for the fast cache-hit path (no contention between threads).
    /// - Disk / network I/O runs entirely outside the lock so other threads
    ///   can continue reading cached tiles concurrently.
    /// - Write lock uses `entry().or_insert()` to safely handle the race where
    ///   two threads both reach the slow path for the same tile; only one copy
    ///   is kept and the other is dropped (both contain identical data).
    fn load_tile(&self, tx: u32, ty: u32) -> Result<TileArc> {
        // Fast path — read lock, no exclusive access needed.
        if let Some(arc) = self.mem.read().unwrap().get(&(tx, ty)) {
            return Ok(arc.clone());
        }

        // Slow path — I/O outside the lock.
        let path = self.tile_path(tx, ty);
        let arc = if path.exists() {
            match load_tile_gz(&path) {
                Ok(g) => Arc::new(g),
                Err(e) => {
                    eprintln!("warn: DEM cache read {tx},{ty}: {e:#} — re-downloading");
                    self.fetch_or_zero(tx, ty)
                }
            }
        } else {
            self.fetch_or_zero(tx, ty)
        };

        // Write lock — or_insert drops the duplicate if another thread won the race.
        Ok(self.mem.write().unwrap().entry((tx, ty)).or_insert(arc).clone())
    }

    /// Download a tile from GSI.
    ///
    /// Returns:
    /// - `Ok(Some(grid))` — tile data successfully downloaded
    /// - `Ok(None)`       — tile does not exist on the server (all URLs 404)
    /// - `Err`            — network or decode error
    fn download(&self, tx: u32, ty: u32) -> Result<Option<TileGrid>> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build();

        let urls = [
            format!("https://cyberjapandata.gsi.go.jp/xyz/dem5a_png/{ZOOM}/{tx}/{ty}.png"),
            format!("https://cyberjapandata.gsi.go.jp/xyz/dem_png/{ZOOM}/{tx}/{ty}.png"),
        ];

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
                    // This URL has no data; try fallback.
                    continue;
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("{e}").context(format!("GET {url}")));
                }
            }
        }
        // All URLs returned 404 — tile genuinely absent from GSI servers.
        Ok(None)
    }

    /// Load or create a tile grid. On 404, saves a zero-filled grid to disk so
    /// the tile is not re-requested on the next run.
    fn fetch_or_zero(&self, tx: u32, ty: u32) -> TileArc {
        match self.download(tx, ty) {
            Ok(Some(g)) => {
                let path = self.tile_path(tx, ty);
                if let Err(e) = save_tile_gz(&path, &g) {
                    eprintln!("warn: DEM cache write {tx},{ty}: {e:#}");
                }
                Arc::new(g)
            }
            Ok(None) => {
                eprintln!("DEM: tile {tx},{ty} not found on server — storing as 0 m");
                let g = Box::new([0f32; GRID_SIZE]);
                let path = self.tile_path(tx, ty);
                if let Err(e) = save_tile_gz(&path, &g) {
                    eprintln!("warn: DEM cache write {tx},{ty}: {e:#}");
                }
                Arc::new(g)
            }
            Err(e) => {
                eprintln!("warn: DEM download {tx},{ty}: {e:#} — storing as 0 m");
                let g = Box::new([0f32; GRID_SIZE]);
                let path = self.tile_path(tx, ty);
                let _ = save_tile_gz(&path, &g);
                Arc::new(g)
            }
        }
    }

    /// Serially download/cache all unique tiles needed for the given positions.
    pub fn prefetch(&self, coords: &[(f64, f64)]) -> Result<()> {
        let mut needed: Vec<(u32, u32)> = coords
            .iter()
            .map(|&(lat, lon)| lat_lon_to_tile(lat, lon))
            .collect();
        needed.sort_unstable();
        needed.dedup();

        for (tx, ty) in needed {
            // Read lock for the fast already-cached check.
            if self.mem.read().unwrap().contains_key(&(tx, ty)) {
                continue;
            }

            // I/O outside the lock.
            let arc = if self.tile_path(tx, ty).exists() {
                match load_tile_gz(&self.tile_path(tx, ty)) {
                    Ok(g) => Arc::new(g),
                    Err(e) => {
                        eprintln!("warn: DEM cache read {tx},{ty}: {e:#} — re-downloading");
                        eprintln!("DEM: downloading tile {tx},{ty}…");
                        self.fetch_or_zero(tx, ty)
                    }
                }
            } else {
                eprintln!("DEM: downloading tile {tx},{ty}…");
                self.fetch_or_zero(tx, ty)
            };

            // Write lock — or_insert is safe even if prefetch is called
            // concurrently (e.g. from multiple rayon threads).
            self.mem.write().unwrap().entry((tx, ty)).or_insert(arc);
        }
        Ok(())
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
}
