fn main() {
    // src-tauri/.env があれば読み込む（なくてもエラーにしない）
    dotenvy::dotenv().ok();
    println!("cargo:rerun-if-changed=.env");

    // 環境変数をコンパイル時定数として渡す（option_env! で参照）
    for var in ["GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"] {
        if let Ok(val) = std::env::var(var) {
            println!("cargo:rustc-env={var}={val}");
        }
        println!("cargo:rerun-if-env-changed={var}");
    }

    #[cfg(target_os = "windows")]
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest")),
    ))
    .expect("failed to run tauri-build");

    #[cfg(not(target_os = "windows"))]
    tauri_build::build();
}
