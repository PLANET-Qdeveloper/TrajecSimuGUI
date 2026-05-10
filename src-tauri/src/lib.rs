use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use tile_cache::aerial::AerialCache;
use tile_cache::dem::DemTileCache;

struct TileCaches {
    aerial: Arc<AerialCache>,
    dem: Arc<DemTileCache>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// ── ファイル I/O ─────────────────────────────────────────────────────────────

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path))
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    // 親ディレクトリが存在しない場合は作成
    if let Some(parent) = PathBuf::from(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, content).map_err(|e| format!("{}: {e}", path))
}

// ── シミュレーション実行 ──────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct SimSummary {
    pub apogee_m: f64,
    pub max_speed_mps: f64,
    pub flight_time_sec: f64,
    pub landing_lat: Option<f64>,
    pub landing_lon: Option<f64>,
    pub landing_alt_m: Option<f64>,
    pub landing_source: Option<String>,
    pub out_dir: String,
}

#[tauri::command]
async fn run_simulation(
    config: simulator_cli::config::Config,
    out_dir: String,
    no_dem: bool,
    app: tauri::AppHandle,
) -> Result<SimSummary, String> {
    let out_path = PathBuf::from(out_dir);

    let result = tauri::async_runtime::spawn_blocking(move || {
        run_simulation_blocking(config, out_path, no_dem, &app)
    })
    .await
    .map_err(|e| format!("スレッドエラー: {e}"))?;

    result
}

fn run_simulation_blocking(
    cfg: simulator_cli::config::Config,
    out_dir: PathBuf,
    no_dem: bool,
    app: &tauri::AppHandle,
) -> Result<SimSummary, String> {
    use simulator_cli::{assemble, dem, refine_landing, runner};

    let emit = |msg: &str| {
        let _ = app.emit("sim-progress", msg);
    };

    emit("パラメータを組み立て中...");
    let params = assemble::assemble(&cfg).map_err(|e| format!("パラメータエラー: {e:#}"))?;

    unsafe {
        std::env::set_var("JSBSIM_DEBUG", "0");
    }
    emit("シミュレーションを実行中...");
    let mut output =
        runner::simulate(&params).map_err(|e| format!("シミュレーションエラー: {e:#}"))?;

    if !no_dem {
        emit("着地点を補正中 (DEM)...");
        match dem::DemCache::new() {
            Ok(cache) => {
                if let Err(e) = refine_landing::refine_one(&mut output, &cache) {
                    log::warn!("DEM 補正失敗、元の着地点を使用: {e:#}");
                }
            }
            Err(e) => log::warn!("DEM キャッシュ初期化失敗: {e:#}"),
        }
    }

    emit("結果を保存中...");
    let _paths = runner::write_outputs(
        &output,
        &out_dir,
        cfg.sim.csv_sample_interval as usize,
        cfg.sim.kml_sample_interval as usize,
        &params,
    )
    .map_err(|e| format!("出力書き込みエラー: {e:#}"))?;

    // サマリ情報を組み立てて返す
    use simulator_cli::EventKind;
    let has_parachute = output.events.iter().any(|e| e.kind == EventKind::ParachuteLanded);
    let (landing_lat, landing_lon, landing_alt_m, landing_source) = if has_parachute {
        output.parachute_branch.trajectory.last_state().map_or(
            (None, None, None, None),
            |s| {
                (
                    Some(s.position.lat_deg),
                    Some(s.position.lon_deg),
                    Some(s.position.alt_msl_m),
                    Some("parachute".to_string()),
                )
            },
        )
    } else {
        output.mainline.trajectory.last_state().map_or((None, None, None, None), |s| {
            (
                Some(s.position.lat_deg),
                Some(s.position.lon_deg),
                Some(s.position.alt_msl_m),
                Some("ballistic".to_string()),
            )
        })
    };

    emit("完了");
    Ok(SimSummary {
        apogee_m: output.mainline.max_altitude_m,
        max_speed_mps: output
            .mainline
            .max_speed_mps
            .max(output.parachute_branch.max_speed_mps),
        flight_time_sec: output
            .mainline
            .flight_time_sec
            .max(output.parachute_branch.flight_time_sec),
        landing_lat,
        landing_lon,
        landing_alt_m,
        landing_source,
        out_dir: out_dir.to_string_lossy().to_string(),
    })
}

// ── Tauri アプリ本体 ──────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let caches = Arc::new(TileCaches {
        aerial: Arc::new(AerialCache::new().expect("aerial cache init failed")),
        dem: Arc::new(DemTileCache::new().expect("dem tile cache init failed")),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("tile", move |_ctx, request, responder| {
            let c = caches.clone();
            std::thread::spawn(move || {
                responder.respond(serve_tile(&c, request));
            });
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            read_text_file,
            write_text_file,
            run_simulation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn serve_tile(
    caches: &TileCaches,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let path = &request.uri().path()[1..];
    let parts: Vec<&str> = path.splitn(4, '/').collect();

    macro_rules! err {
        ($status:expr, $msg:expr) => {
            tauri::http::Response::builder()
                .status($status)
                .header("Access-Control-Allow-Origin", "*")
                .body($msg.as_bytes().to_vec())
                .unwrap()
        };
    }

    if parts.len() != 4 {
        return err!(400, "expected /{kind}/{z}/{x}/{y}");
    }
    let (Ok(z), Ok(x), Ok(y)) = (
        parts[1].parse::<u8>(),
        parts[2].parse::<u32>(),
        parts[3].parse::<u32>(),
    ) else {
        return err!(400, "invalid tile coordinates");
    };

    match parts[0] {
        "aerial" => match caches.aerial.get_tile(z, x, y) {
            Ok(Some(jpeg)) => tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", "image/jpeg")
                .header("Access-Control-Allow-Origin", "*")
                .header("Cache-Control", "public, max-age=86400")
                .body((*jpeg).clone())
                .unwrap(),
            Ok(None) => err!(404, "tile not found"),
            Err(e) => {
                log::error!("aerial tile {z}/{x}/{y}: {e:#}");
                err!(500, "tile cache error")
            }
        },
        "dem" => match caches.dem.get_tile(z, x, y) {
            Ok(Some(png)) => tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", "image/png")
                .header("Access-Control-Allow-Origin", "*")
                .header("Cache-Control", "public, max-age=86400")
                .body((*png).clone())
                .unwrap(),
            Ok(None) => err!(404, "dem tile not found"),
            Err(e) => {
                log::error!("dem tile {z}/{x}/{y}: {e:#}");
                err!(500, "dem tile cache error")
            }
        },
        _ => err!(400, "unknown tile kind"),
    }
}
