use serde::Serialize;
use simulator_cli::kml_writer::write_trajectory_kml;
use simulator_cli::EventKind;
use simulator_cli::{assemble, dem, refine_landing, runner};
use std::cmp::min;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
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

// ── Config 読み込み / 保存 / バリデーション ───────────────────────────────────

#[tauri::command]
fn load_config(path: String) -> Result<simulator_cli::config::Config, String> {
    simulator_cli::config::Config::load(std::path::Path::new(&path)).map_err(|e| format!("{e:#}"))
}

fn to_relative(base: &std::path::Path, abs: &std::path::Path) -> PathBuf {
    let bc: Vec<_> = base.components().collect();
    let ac: Vec<_> = abs.components().collect();
    let n = bc.iter().zip(&ac).take_while(|(a, b)| a == b).count();
    let mut r = PathBuf::new();
    for _ in 0..(bc.len() - n) {
        r.push("..");
    }
    for c in &ac[n..] {
        r.push(c);
    }
    r
}

#[tauri::command]
fn save_config(mut config: simulator_cli::config::Config, save_path: String) -> Result<(), String> {
    let save_dir = Path::new(&save_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let rel = |p: &mut PathBuf| {
        if !p.as_os_str().is_empty() {
            *p = to_relative(&save_dir, p);
        }
    };

    rel(&mut config.engine.thrust_table);
    rel(&mut config.aero.cp_mach_table);
    rel(&mut config.aero.cd0_alpha_mach_table);
    rel(&mut config.aero.cn_table);
    rel(&mut config.aero.cs_table);
    if let Some(p) = config.parachute.as_mut() {
        rel(&mut p.terminal_velocity_table);
    }
    if let Some(w) = config.launch.wind_table.as_mut() {
        rel(w);
    }

    let yaml =
        serde_yaml::to_string(&config).map_err(|e| format!("YAML シリアライズエラー: {e}"))?;
    if let Some(parent) = std::path::Path::new(&save_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&save_path, yaml).map_err(|e| format!("{save_path}: {e}"))
}

#[tauri::command]
fn validate_config(config: simulator_cli::config::Config) -> Result<(), String> {
    simulator_cli::assemble::assemble(&config)
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}

// ── シミュレーション実行 ──────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct SimSummary {
    pub apogee_m: f64,
    pub max_speed_mps: f64,
    pub flight_time_sec: f64,
    pub landing_lat_parachute: Option<f64>,
    pub landing_lon_parachute: Option<f64>,
    pub landing_alt_m_parachute: Option<f64>,
    pub landing_lat_ballistic: Option<f64>,
    pub landing_lon_ballistic: Option<f64>,
    pub landing_alt_m_ballistic: Option<f64>,
    pub kml_result: String,
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

    let has_parachute = output
        .events
        .iter()
        .any(|e| e.kind == EventKind::ParachuteLanded);
    let (landing_lat_parachute, landing_lon_parachute, landing_alt_m_parachute) = if has_parachute {
        output
            .parachute_branch
            .trajectory
            .last_state()
            .map_or((None, None, None), |s| {
                (
                    Some(s.position.lat_deg),
                    Some(s.position.lon_deg),
                    Some(s.position.alt_msl_m),
                )
            })
    } else {
        (None, None, None)
    };
    let (landing_lat_ballistic, landing_lon_ballistic, landing_alt_m_ballistic) = output
        .mainline
        .trajectory
        .last_state()
        .map(|s| {
            (
                Some(s.position.lat_deg),
                Some(s.position.lon_deg),
                Some(s.position.alt_msl_m),
            )
        })
        .unwrap_or((None, None, None));

    let mut kml_vector = Vec::new();
    write_trajectory_kml(
        &output,
        (cfg.sim.kml_sample_interval
            / min(cfg.sim.kml_sample_interval, cfg.sim.csv_sample_interval)) as usize,
        kml_vector.as_ref(),
    )
    .map_err(|e| format!("KML 生成エラー: {e:#}"))?;
    let kml_string = String::from_utf8(kml_vector).map_err(|e| format!("KML 生成エラー: {e:#}"))?;

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
        landing_lat_ballistic,
        landing_lon_ballistic,
        landing_alt_m_ballistic,
        landing_lat_parachute,
        landing_lon_parachute,
        landing_alt_m_parachute,
        kml_result: kml_string,
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
            load_config,
            save_config,
            validate_config,
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
