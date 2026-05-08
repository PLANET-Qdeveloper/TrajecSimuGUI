//! GSI seamlessphoto tile cache (aerial imagery, JPEG).
//!
//! Tiles are fetched from cyberjapandata.gsi.go.jp, stored as raw JPEG bytes
//! in an MBTiles SQLite database at `{cache_dir}/trajec_simu_dem/aerial.mbtiles`.
//! Schema mirrors dem.mbtiles; tile_data contains raw JPEG bytes (no additional compression).
//!
//! TMS y-axis convention: tile_row = (2^zoom - 1) - ty.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

const BASE_URL: &str = "https://cyberjapandata.gsi.go.jp/xyz/seamlessphoto";

type TileKey = (u8, u32, u32); // (zoom, tx, ty)
type TileArc = Arc<Vec<u8>>;   // raw JPEG bytes

pub struct AerialCache {
    db:  Mutex<Connection>,
    mem: RwLock<HashMap<TileKey, TileArc>>,
}

impl AerialCache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().context("cannot find OS cache dir")?;
        let dir  = base.join("trajec_simu_dem");
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("aerial.mbtiles");
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

    /// Returns raw JPEG bytes for the tile, or None if the tile does not exist (404).
    /// Three-layer cache: memory → MBTiles DB → GSI network.
    pub fn get_tile(&self, zoom: u8, tx: u32, ty: u32) -> Result<Option<Arc<Vec<u8>>>> {
        // Fast path: memory cache (read lock, no contention between threads).
        if let Some(arc) = self.mem.read().unwrap().get(&(zoom, tx, ty)) {
            return Ok(Some(arc.clone()));
        }

        // Slow path: DB lookup outside mem lock.
        let arc = match self.load_tile_db(zoom, tx, ty)? {
            Some(bytes) => Arc::new(bytes),
            None => match self.download(zoom, tx, ty)? {
                Some(bytes) => {
                    if let Err(e) = self.save_tile_db(zoom, tx, ty, &bytes) {
                        log::warn!("aerial cache write {zoom}/{tx}/{ty}: {e:#}");
                    }
                    Arc::new(bytes)
                }
                None => return Ok(None),
            },
        };

        // Write lock: or_insert handles the race if two threads fetched the same tile.
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

    fn download(&self, zoom: u8, tx: u32, ty: u32) -> Result<Option<Vec<u8>>> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build();
        let url = format!("{BASE_URL}/{zoom}/{tx}/{ty}.jpg");
        match agent.get(&url).call() {
            Ok(resp) => {
                let mut bytes = Vec::new();
                resp.into_reader()
                    .read_to_end(&mut bytes)
                    .context("reading aerial tile response")?;
                Ok(Some(bytes))
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("{e}").context(format!("GET {url}"))),
        }
    }
}
