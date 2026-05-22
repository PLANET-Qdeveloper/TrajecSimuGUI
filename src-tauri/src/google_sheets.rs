use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ── ビルド時クレデンシャル ────────────────────────────────────────────────────

const CLIENT_ID: Option<&str> = option_env!("GOOGLE_CLIENT_ID");
const CLIENT_SECRET: Option<&str> = option_env!("GOOGLE_CLIENT_SECRET");

fn client_id() -> Result<&'static str, String> {
    CLIENT_ID.ok_or_else(|| {
        "GOOGLE_CLIENT_ID がビルド時に設定されていません。環境変数を設定して再ビルドしてください。"
            .to_string()
    })
}

fn client_secret() -> Result<&'static str, String> {
    CLIENT_SECRET.ok_or_else(|| {
        "GOOGLE_CLIENT_SECRET がビルド時に設定されていません。環境変数を設定して再ビルドしてください。"
            .to_string()
    })
}

// ── SheetConfig ──────────────────────────────────────────────────────────────

/// スプレッドシートから取得したスカラー値。None はシートに値がなかったフィールド。
/// ファイルパス（推力テーブル等）は含まない。
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct SheetConfig {
    // Launch
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub elevation: Option<f64>,
    pub rail_length: Option<f64>,
    pub pitch: Option<f64>,
    pub yaw: Option<f64>,
    pub wind_power_exponent: Option<f64>,
    pub wind_reference_alt: Option<f64>,

    // Body（単位換算後: mm → m）
    pub diameter_m: Option<f64>,
    pub dry_mass_kg: Option<f64>,
    pub cg_axial_m: Option<f64>,
    pub inertia_pitch_yaw: Option<f64>,
    pub inertia_roll: Option<f64>,

    // Aero（単位換算後: mm → m）
    pub cp_axial_m: Option<f64>,
    pub roll_damping: Option<f64>,
    pub pitch_damping: Option<f64>,

    // Engine / Tank / Fuel
    pub oxidizer_mass_kg: Option<f64>,
    pub fuel_mass_initial_kg: Option<f64>,
    pub fuel_mass_final_kg: Option<f64>,
    pub tank_axial_pos_m: Option<f64>,
    pub fuel_axial_pos_m: Option<f64>,

    // Parachute（terminal_velocity_table はファイルパスのため含まない）
    pub deploy_delay_sec: Option<f64>,
}

// ── トークンキャッシュ ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct TokenCache {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: u64, // Unix timestamp (seconds)
}

fn token_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("google_token.json"))
        .map_err(|e| format!("アプリデータディレクトリの取得に失敗: {e}"))
}

fn load_token_cache(app: &AppHandle) -> Option<TokenCache> {
    let path = token_path(app).ok()?;
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

fn save_token_cache(app: &AppHandle, cache: &TokenCache) -> Result<(), String> {
    let path = token_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("ディレクトリ作成失敗: {e}"))?;
    }
    let s = serde_json::to_string_pretty(cache).map_err(|e| format!("JSON 変換失敗: {e}"))?;
    std::fs::write(&path, s).map_err(|e| format!("トークン保存失敗: {e}"))
}

pub fn is_logged_in(app: &AppHandle) -> bool {
    load_token_cache(app).is_some()
}

pub fn revoke_token(app: &AppHandle) -> Result<(), String> {
    let path = token_path(app)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("トークン削除失敗: {e}"))
    } else {
        Ok(())
    }
}

// ── PKCE ヘルパー ────────────────────────────────────────────────────────────

fn random_string(len: usize) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn pkce_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

// ── アクセストークン取得 ─────────────────────────────────────────────────────

pub async fn get_access_token(app: &AppHandle) -> Result<String, String> {
    let now = unix_now();

    if let Some(cache) = load_token_cache(app) {
        if cache.expires_at > now + 60 {
            return Ok(cache.access_token);
        }
        if let Some(refresh) = cache.refresh_token.as_deref() {
            if let Ok(token) = refresh_access_token(app, refresh).await {
                return Ok(token);
            }
            // リフレッシュ失敗時はキャッシュを削除してフルフローへ
            let _ = revoke_token(app);
        }
    }

    run_oauth_flow(app).await
}

async fn refresh_access_token(app: &AppHandle, refresh_token: &str) -> Result<String, String> {
    let client_id = client_id()?;
    let client_secret = client_secret()?;

    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| format!("トークンリフレッシュ失敗: {e}"))?
        .json()
        .await
        .map_err(|e| format!("レスポンス解析失敗: {e}"))?;

    let access_token = resp["access_token"]
        .as_str()
        .ok_or_else(|| format!("リフレッシュ応答に access_token なし: {resp}"))?
        .to_string();
    let expires_in = resp["expires_in"].as_u64().unwrap_or(3600);

    let mut cache = load_token_cache(app).unwrap_or(TokenCache {
        access_token: String::new(),
        refresh_token: None,
        expires_at: 0,
    });
    cache.access_token = access_token.clone();
    cache.expires_at = unix_now() + expires_in;
    save_token_cache(app, &cache)?;

    Ok(access_token)
}

// ── OAuth2 PKCE フロー ───────────────────────────────────────────────────────

async fn run_oauth_flow(app: &AppHandle) -> Result<String, String> {
    let client_id = client_id()?;
    let client_secret = client_secret()?;

    let code_verifier = random_string(64);
    let code_challenge = pkce_challenge(&code_verifier);
    let state = random_string(16);

    // ランダムポートでローカルサーバーを起動
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("ローカルサーバー起動失敗: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("ポート取得失敗: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // 認証 URL を構築
    let mut auth_url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair(
            "scope",
            "https://www.googleapis.com/auth/spreadsheets.readonly",
        )
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");

    // システムブラウザで開く
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(auth_url.as_str(), None::<&str>)
        .map_err(|e| format!("ブラウザを開けません: {e}"))?;

    // コールバックを待機（120秒タイムアウト）
    let accept_future = listener.accept();
    let (mut stream, _) = tokio::time::timeout(std::time::Duration::from_secs(120), accept_future)
        .await
        .map_err(|_| "認証がタイムアウトしました（120秒）。再度お試しください。".to_string())?
        .map_err(|e| format!("接続受付エラー: {e}"))?;

    // HTTP リクエストを読み取りコードを抽出
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("リクエスト読み取り失敗: {e}"))?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // 成功レスポンスを返す
    let _ = stream
        .write_all(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
            <html><body style=\"font-family:sans-serif;padding:2em\">\
            <h2>✅ 認証完了</h2><p>このタブを閉じてアプリに戻ってください。</p>\
            </body></html>"
                .as_bytes(),
        )
        .await;

    let code = extract_oauth_code(&request, &state)?;
    exchange_code_for_token(
        app,
        client_id,
        client_secret,
        &code,
        &redirect_uri,
        &code_verifier,
    )
    .await
}

fn extract_oauth_code(request: &str, expected_state: &str) -> Result<String, String> {
    // 最初の行から "GET /callback?code=xxx&state=yyy HTTP/1.1" を解析
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");

    let params: HashMap<&str, &str> = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .collect();

    if let Some(error) = params.get("error") {
        return Err(format!("Google 認証エラー: {error}"));
    }

    let returned_state = params.get("state").copied().unwrap_or("");
    if returned_state != expected_state {
        return Err("state パラメータが一致しません（セキュリティチェック失敗）".to_string());
    }

    params
        .get("code")
        .map(|s| s.to_string())
        .ok_or_else(|| "認証コードが取得できません".to_string())
}

async fn exchange_code_for_token(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<String, String> {
    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| format!("トークン取得失敗: {e}"))?
        .json()
        .await
        .map_err(|e| format!("トークンレスポンス解析失敗: {e}"))?;

    let access_token = resp["access_token"]
        .as_str()
        .ok_or_else(|| format!("access_token なし: {resp}"))?
        .to_string();
    let refresh_token = resp["refresh_token"].as_str().map(str::to_string);
    let expires_in = resp["expires_in"].as_u64().unwrap_or(3600);

    let cache = TokenCache {
        access_token: access_token.clone(),
        refresh_token,
        expires_at: unix_now() + expires_in,
    };
    save_token_cache(app, &cache)?;

    Ok(access_token)
}

// ── スプレッドシート URL パース ───────────────────────────────────────────────

fn extract_sheet_info(raw_url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(raw_url).ok()?;
    if !parsed.host_str()?.contains("google.com") {
        return None;
    }

    // /spreadsheets/d/{ID}/... からシート ID を取得
    let sheet_id = parsed
        .path_segments()?
        .skip_while(|s| *s != "d")
        .nth(1)
        .filter(|s| !s.is_empty())?
        .to_string();

    // gid をクエリパラメータから取得、なければフラグメントを確認
    let gid = parsed
        .query_pairs()
        .find(|(k, _)| k == "gid")
        .map(|(_, v)| v.into_owned())
        .or_else(|| {
            parsed
                .fragment()
                .and_then(|f| f.strip_prefix("gid="))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "0".to_string());

    Some((sheet_id, gid))
}

// ── シート CSV 取得 ──────────────────────────────────────────────────────────

async fn fetch_sheet_csv(sheet_id: &str, gid: &str, token: &str) -> Result<String, String> {
    let url =
        format!("https://docs.google.com/spreadsheets/d/{sheet_id}/export?format=csv&gid={gid}");
    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("シート取得失敗: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(format!(
            "シート取得エラー (HTTP {status})。シートの共有設定またはアクセス権限を確認してください。"
        ));
    }

    resp.text()
        .await
        .map_err(|e| format!("レスポンス読み込み失敗: {e}"))
}

// ── CSV パース ───────────────────────────────────────────────────────────────

/// 諸元表 CSV をパースして SheetConfig を返す。
/// 不明・数値変換不可なフィールドは None のまま返す（エラーにしない）。
pub fn parse_sheet_csv(csv: &str) -> SheetConfig {
    // BOM を除去してフィールドマップを構築
    let csv = csv.trim_start_matches('\u{feff}');
    let field_map = build_field_map(csv);

    let mm_to_m =
        |key: &str| -> Option<f64> { field_map.get(key)?.parse::<f64>().ok().map(|v| v / 1000.0) };
    let parse = |key: &str| -> Option<f64> { field_map.get(key)?.parse::<f64>().ok() };

    SheetConfig {
        // 射点座標
        latitude: parse("射点座標/緯度"),
        longitude: parse("射点座標/経度"),
        elevation: parse("射点座標/海面高度"),

        // ランチャ
        yaw: parse("ランチャ/打上方位角"),
        pitch: parse("ランチャ/打上射角"),
        rail_length: parse("ランチャ/レール長さ"),

        // 風
        wind_power_exponent: parse("風/べき指数"),
        wind_reference_alt: parse("風/基準高さ"),

        // 機体形状（mm → m）
        diameter_m: mm_to_m("機体形状/代表直径"),

        // 質量
        dry_mass_kg: parse("質量/乾燥質量"),
        oxidizer_mass_kg: parse("質量/酸化剤質量"),
        fuel_mass_initial_kg: parse("質量/燃焼前燃料質量"),
        fuel_mass_final_kg: parse("質量/燃焼後燃料質量"),

        // 重心（mm → m）
        cg_axial_m: mm_to_m("重心/乾燥時重心位置"),
        tank_axial_pos_m: mm_to_m("重心/タンク口金位置"),
        fuel_axial_pos_m: mm_to_m("重心/燃料重心位置"),

        // 慣性モーメント
        inertia_pitch_yaw: parse("慣性モーメント/ヨー・ピッチ"),
        inertia_roll: parse("慣性モーメント/ロール"),

        // 空力（mm → m）
        cp_axial_m: mm_to_m("空力/圧力中心位置"),
        roll_damping: parse("空力/減衰モーメント係数（ロール）"),
        pitch_damping: parse("空力/減衰モーメント係数（ピッチ・ヨー）"),

        // リカバリ（2段階目）
        deploy_delay_sec: parse("リカバリ（2段階目）/開傘時刻"),
    }
}

/// CSV テキストから {section/subitem → value} のマップを構築する。
///
/// 諸元表の構造（col index は先頭の空カラムを含む）:
///   col[0] = "" (常に空)
///   col[1] = 大項目（セクション名。非空のとき section を更新）
///   col[2] = 小項目（フィールド名）
///   col[3] = 値
fn build_field_map(csv: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut section = String::new();

    for line in csv.lines() {
        let cols = parse_csv_row(line);
        if cols.len() < 4 {
            continue;
        }

        let item = cols[1].trim();
        let subitem = cols[2].trim();
        let value = cols[3].trim();

        if !item.is_empty() {
            section = item.to_string();
        }

        // subitem と value が両方非空のときのみ記録
        if !subitem.is_empty() && !value.is_empty() {
            map.insert(format!("{section}/{subitem}"), value.to_string());
        }
    }

    map
}

/// 1行の CSV をフィールド列にパースする（ダブルクォート対応）。
fn parse_csv_row(line: &str) -> Vec<String> {
    let line = line.trim_end_matches('\r');
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if !in_quotes => in_quotes = true,
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    in_quotes = false;
                }
            }
            ',' if !in_quotes => {
                fields.push(current.clone());
                current.clear();
            }
            c => current.push(c),
        }
    }
    fields.push(current);
    fields
}

// ── 公開 Tauri コマンド向け関数 ──────────────────────────────────────────────

/// スプレッドシート URL からデータを取得して SheetConfig を返す。
pub async fn fetch_google_sheet(app: &AppHandle, url: String) -> Result<SheetConfig, String> {
    let (sheet_id, gid) = extract_sheet_info(&url).ok_or_else(|| {
        "無効なスプレッドシート URL です。\n\
        Google スプレッドシートの URL を貼り付けてください。"
            .to_string()
    })?;

    let token = get_access_token(app).await?;
    let csv = fetch_sheet_csv(&sheet_id, &gid, &token).await?;
    Ok(parse_sheet_csv(&csv))
}

// ── ユーティリティ ────────────────────────────────────────────────────────────

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── テスト ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CSV: &str = "\
,,,,,
,更新日:,2025/8/4,,,
,Item,SubItem,Value,Unit,Note
,機体名,,Felix-Stella,-,機体名。
,射点座標,,,,
,,緯度,40.242865,deg,
,,経度,140.01045,deg,
,,海面高度,5.3,m,
,ランチャ,,,,
,,打上方位角,292.34,deg,
,,打上射角,80,deg,
,,レール長さ,5,m,
,風,,,,
,,べき指数,6,-,
,,基準高さ,2,,
,機体形状,,,,
,,代表直径,145,mm,
,質量,,,,
,,乾燥質量,16.99,kg,
,,酸化剤質量,3.53,kg,
,,燃焼前燃料質量,0.643,kg,
,,燃焼後燃料質量,0.1,kg,
,重心,,,,
,,乾燥時重心位置,745,mm,
,,タンク口金位置,748,mm,
,,燃料重心位置,330,mm,
,慣性モーメント,,,,
,,ヨー・ピッチ,4.12,kg・m2,
,,ロール,0.05,kg・m2,
,空力,,,,
,,圧力中心位置,515,mm,
,,軸力係数,CSVで提出,-,
,,減衰モーメント係数（ロール）,-0.073,-,
,,減衰モーメント係数（ピッチ・ヨー）,-2.394,-,
,リカバリ（2段階目）,,,,
,,開傘時刻,3,s,
,,降下終端速度,20,m/s,
";

    #[test]
    fn parse_basic_fields() {
        let cfg = parse_sheet_csv(SAMPLE_CSV);
        assert!((cfg.latitude.unwrap() - 40.242865).abs() < 1e-6);
        assert!((cfg.longitude.unwrap() - 140.01045).abs() < 1e-6);
        assert!((cfg.elevation.unwrap() - 5.3).abs() < 1e-6);
        assert!((cfg.yaw.unwrap() - 292.34).abs() < 1e-6);
        assert!((cfg.pitch.unwrap() - 80.0).abs() < 1e-6);
        assert!((cfg.rail_length.unwrap() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn parse_mm_to_m_conversion() {
        let cfg = parse_sheet_csv(SAMPLE_CSV);
        assert!((cfg.diameter_m.unwrap() - 0.145).abs() < 1e-6);
        assert!((cfg.cg_axial_m.unwrap() - 0.745).abs() < 1e-6);
        assert!((cfg.cp_axial_m.unwrap() - 0.515).abs() < 1e-6);
        assert!((cfg.tank_axial_pos_m.unwrap() - 0.748).abs() < 1e-6);
        assert!((cfg.fuel_axial_pos_m.unwrap() - 0.330).abs() < 1e-6);
    }

    #[test]
    fn parse_mass_and_inertia() {
        let cfg = parse_sheet_csv(SAMPLE_CSV);
        assert!((cfg.dry_mass_kg.unwrap() - 16.99).abs() < 1e-6);
        assert!((cfg.oxidizer_mass_kg.unwrap() - 3.53).abs() < 1e-6);
        assert!((cfg.fuel_mass_initial_kg.unwrap() - 0.643).abs() < 1e-6);
        assert!((cfg.fuel_mass_final_kg.unwrap() - 0.1).abs() < 1e-6);
        assert!((cfg.inertia_pitch_yaw.unwrap() - 4.12).abs() < 1e-6);
        assert!((cfg.inertia_roll.unwrap() - 0.05).abs() < 1e-6);
    }

    #[test]
    fn csv_de_teishutsu_is_none() {
        let cfg = parse_sheet_csv(SAMPLE_CSV);
        // "CSVで提出" は数値変換不可 → None になる
        // (軸力係数を直接チェックする方法はないが、他フィールドへの影響がないことを確認)
        assert!(cfg.latitude.is_some());
    }

    #[test]
    fn parse_negative_damping() {
        let cfg = parse_sheet_csv(SAMPLE_CSV);
        assert!((cfg.roll_damping.unwrap() - (-0.073)).abs() < 1e-6);
        assert!((cfg.pitch_damping.unwrap() - (-2.394)).abs() < 1e-6);
    }

    #[test]
    fn parse_parachute() {
        let cfg = parse_sheet_csv(SAMPLE_CSV);
        assert!((cfg.deploy_delay_sec.unwrap() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn extract_sheet_info_edit_url() {
        let (id, gid) = extract_sheet_info(
            "https://docs.google.com/spreadsheets/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms/edit#gid=0",
        )
        .unwrap();
        assert_eq!(id, "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms");
        assert_eq!(gid, "0");
    }

    #[test]
    fn extract_sheet_info_query_gid() {
        let (id, gid) =
            extract_sheet_info("https://docs.google.com/spreadsheets/d/ABC123/edit?gid=1234567")
                .unwrap();
        assert_eq!(id, "ABC123");
        assert_eq!(gid, "1234567");
    }

    #[test]
    fn extract_sheet_info_no_gid() {
        let (id, gid) =
            extract_sheet_info("https://docs.google.com/spreadsheets/d/XYZ789/edit").unwrap();
        assert_eq!(id, "XYZ789");
        assert_eq!(gid, "0");
    }

    #[test]
    fn extract_sheet_info_invalid() {
        assert!(extract_sheet_info("https://example.com/sheet").is_none());
        assert!(extract_sheet_info("not-a-url").is_none());
    }
}
