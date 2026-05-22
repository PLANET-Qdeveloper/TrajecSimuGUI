mod google_sheets;

use serde::Serialize;
use simulator_cli::kml_writer::write_trajectory_kml;
use simulator_cli::pipeline::PostProcessor;
use simulator_cli::EventKind;
use simulator_cli::{assemble, dem, landing_area, pipeline, refine_landing, simulate};
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
fn load_kml_file(path: String) -> Result<String, String> {
    use std::io::Read;
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "kml" => std::fs::read_to_string(p).map_err(|e| format!("{e}")),
        "kmz" => {
            let file = std::fs::File::open(p).map_err(|e| format!("{e}"))?;
            let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("{e}"))?;
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i).map_err(|e| format!("{e}"))?;
                if entry.name().ends_with(".kml") {
                    let mut contents = String::new();
                    entry
                        .read_to_string(&mut contents)
                        .map_err(|e| format!("{e}"))?;
                    return Ok(contents);
                }
            }
            Err("KMZ ファイル内に .kml が見つかりません".to_string())
        }
        _ => Err(format!("未対応の拡張子: {ext}")),
    }
}

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
    let cfg = simulator_cli::config::Config::load(std::path::Path::new(&path))
        .map_err(|e| format!("{e:#}"))?;
    assemble::assemble(&cfg).map_err(|e| format!("{e:#}"))?;
    Ok(cfg)
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
pub struct LandingAreaSummary {
    pub out_dir: String,
    pub kml_result: String,
}

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
        simulate::simulate(&params).map_err(|e| format!("シミュレーションエラー: {e:#}"))?;

    if !no_dem {
        emit("着地点を補正中 (DEM)...");
        let dem_cache = dem::DemCache::new().ok();
        refine_landing::try_refine(&mut output, dem_cache.as_ref());
    }

    emit("結果を保存中...");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("出力ディレクトリ作成エラー: {e}"))?;
    let (csv_int, kml_int) = pipeline::normalise_intervals(
        cfg.sim.csv_sample_interval as usize,
        cfg.sim.kml_sample_interval as usize,
    );
    let ctx = pipeline::RunContext {
        output: &output,
        out_dir: &out_dir,
        params: &params,
        csv_interval: csv_int,
        kml_interval: kml_int,
    };
    let optional_step: Vec<Box<dyn PostProcessor>> = vec![Box::new(pipeline::DrawChartsStep)];
    pipeline::run_pipeline(&ctx, &pipeline::default_mandatory_steps(), &optional_step)
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
    write_trajectory_kml(&output, kml_int, &mut kml_vector)
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

// ── 着地範囲スイープ ──────────────────────────────────────────────────────────

#[tauri::command]
async fn run_landing_area(
    config: simulator_cli::config::Config,
    out_dir: String,
    no_dem: bool,
    directions: u32,
    speed_max: f64,
    speed_steps: u32,
    jobs: Option<usize>,
    app: tauri::AppHandle,
) -> Result<LandingAreaSummary, String> {
    let out_path = PathBuf::from(out_dir);
    let result = tauri::async_runtime::spawn_blocking(move || {
        run_landing_area_blocking(
            config,
            out_path,
            no_dem,
            directions,
            speed_max,
            speed_steps,
            jobs,
            &app,
        )
    })
    .await
    .map_err(|e| format!("スレッドエラー: {e}"))?;
    result
}

fn run_landing_area_blocking(
    cfg: simulator_cli::config::Config,
    out_dir: PathBuf,
    no_dem: bool,
    directions: u32,
    speed_max: f64,
    speed_steps: u32,
    jobs: Option<usize>,
    app: &tauri::AppHandle,
) -> Result<LandingAreaSummary, String> {
    let emit = |msg: &str| {
        let _ = app.emit("sim-progress", msg);
    };

    emit("パラメータを組み立て中...");
    let params = assemble::assemble(&cfg).map_err(|e| format!("パラメータエラー: {e:#}"))?;

    std::fs::create_dir_all(&out_dir).map_err(|e| format!("出力ディレクトリ作成エラー: {e}"))?;

    let (csv_int, kml_int) = pipeline::normalise_intervals(
        cfg.sim.csv_sample_interval as usize,
        cfg.sim.kml_sample_interval as usize,
    );
    let app_clone = app.clone();
    let args = landing_area::LandingAreaArgs {
        out_dir: out_dir.clone(),
        directions,
        speed_max,
        speed_steps,
        jobs,
        csv_interval: csv_int,
        kml_interval: kml_int,
        no_dem,
        on_progress: Some(Arc::new(move |n: usize, total: usize| {
            let _ = app_clone.emit("sim-progress", format!("着地範囲: {n}/{total} 完了"));
        })),
    };

    emit("着地範囲シミュレーション開始...");
    landing_area::run(&cfg, &params, &args).map_err(|e| format!("着地範囲エラー: {e:#}"))?;

    let kml_result = std::fs::read_to_string(out_dir.join("landing_range.kml"))
        .map_err(|e| format!("KML 読み込みエラー: {e}"))?;

    emit("完了");
    Ok(LandingAreaSummary {
        out_dir: out_dir.to_string_lossy().to_string(),
        kml_result,
    })
}

// ── Google スプレッドシート取込 ───────────────────────────────────────────────

#[tauri::command]
async fn fetch_google_sheet(
    app: tauri::AppHandle,
    url: String,
) -> Result<google_sheets::SheetConfig, String> {
    google_sheets::fetch_google_sheet(&app, url).await
}

#[tauri::command]
fn get_google_auth_status(app: tauri::AppHandle) -> bool {
    google_sheets::is_logged_in(&app)
}

#[tauri::command]
fn revoke_google_auth(app: tauri::AppHandle) -> Result<(), String> {
    google_sheets::revoke_token(&app)
}

// ── Tauri アプリ本体 ──────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let caches = Arc::new(TileCaches {
        aerial: Arc::new(AerialCache::new().expect("aerial cache init failed")),
        dem: Arc::new(DemTileCache::new().expect("dem tile cache init failed")),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
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
            load_kml_file,
            load_config,
            save_config,
            validate_config,
            read_text_file,
            write_text_file,
            run_simulation,
            run_landing_area,
            fetch_google_sheet,
            get_google_auth_status,
            revoke_google_auth,
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
