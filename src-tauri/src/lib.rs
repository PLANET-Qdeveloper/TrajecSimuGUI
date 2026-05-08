use std::sync::Arc;
use tile_cache::aerial::AerialCache;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let aerial = Arc::new(AerialCache::new().expect("aerial cache init failed"));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("tile", move |_ctx, request, responder| {
            let cache = aerial.clone();
            std::thread::spawn(move || {
                responder.respond(serve_tile(&cache, request));
            });
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn serve_tile(
    cache: &AerialCache,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    // URL path: "/aerial/{z}/{x}/{y}" — strip leading '/'.
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

    if parts.len() != 4 || parts[0] != "aerial" {
        return err!(400, "expected /aerial/{z}/{x}/{y}");
    }
    let (Ok(z), Ok(x), Ok(y)) = (
        parts[1].parse::<u8>(),
        parts[2].parse::<u32>(),
        parts[3].parse::<u32>(),
    ) else {
        return err!(400, "invalid tile coordinates");
    };

    match cache.get_tile(z, x, y) {
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
    }
}
