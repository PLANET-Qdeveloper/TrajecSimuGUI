//! Live serial telemetry ingestion for the "テレメトリ" tab: up to a few
//! simultaneously-open serial ports, each running its own blocking reader
//! thread that decodes either the main board's binary frames or the valve
//! board's CSV lines (`ref/NSE2026_PROTOCOL.md`) and streams decoded
//! samples to the frontend as Tauri events.

use serde::{Deserialize, Serialize};
use serialport::SerialPort;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};
use telemetry_protocol::{
    decode_main_payload, pressure_to_altitude_m, split_csv_columns, FrameDecoder, FrameError,
    LinkCounters, TlvError, ValveLineBuffer, MAIN_DESTINATION_ID, MAIN_SOURCE_ID,
};

/// An open serial connection. Almost always [`PortHandle::Native`] (the
/// `serialport` crate, which also handles real baud-rate configuration).
///
/// On macOS/iOS, `serialport` always calls the `IOSSIOSPEED` ioctl while
/// opening a port, and that ioctl unconditionally fails with `ENOTTY` when
/// the path is a pseudo-terminal rather than a real serial device — which
/// is exactly what the local NSE2026 mock client (`/dev/ttysNNN`) uses. A
/// PTY has no real baud rate to configure in the first place (the protocol
/// doc notes "PTYではbaud rateそのものは通信速度を変えません"), so when the
/// native open fails, [`open_port_handle`] falls back to opening the fd
/// directly and configuring raw mode without touching the baud rate.
enum PortHandle {
    Native(Box<dyn SerialPort>),
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    RawTty(std::fs::File),
}

impl Read for PortHandle {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            PortHandle::Native(p) => p.read(buf),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            PortHandle::RawTty(f) => f.read(buf),
        }
    }
}

impl Write for PortHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            PortHandle::Native(p) => p.write(buf),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            PortHandle::RawTty(f) => f.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            PortHandle::Native(p) => p.flush(),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            PortHandle::RawTty(f) => f.flush(),
        }
    }
}

impl PortHandle {
    fn try_clone(&self) -> std::io::Result<PortHandle> {
        match self {
            PortHandle::Native(p) => p
                .try_clone()
                .map(PortHandle::Native)
                .map_err(std::io::Error::from),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            PortHandle::RawTty(f) => f.try_clone().map(PortHandle::RawTty),
        }
    }
}

/// Opens `path`, preferring the `serialport` crate's normal handling
/// (real baud rate, parity, etc.) and falling back to a raw-fd PTY open
/// (see [`PortHandle`]) on macOS/iOS if that fails.
fn open_port_handle(path: &str, baud_rate: u32, timeout: Duration) -> Result<PortHandle, String> {
    let native_err = match serialport::new(path, baud_rate).timeout(timeout).open() {
        Ok(p) => return Ok(PortHandle::Native(p)),
        Err(e) => e,
    };

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        // Only fall back for the specific ENOTTY failure the PTY workaround
        // exists for (see the `PortHandle` docs) — any other native-open
        // error (wrong path, device busy, permission denied, ...) should
        // surface as-is rather than silently retrying via a raw open that
        // never configures a baud rate. `serialport`'s posix backend maps
        // ENOTTY to `ErrorKind::Unknown` with the libc strerror text ("Not
        // a typewriter"), which is the only signal its public `Error` type
        // exposes for this case.
        let is_enotty = native_err.kind == serialport::ErrorKind::Unknown
            && native_err
                .description
                .to_ascii_lowercase()
                .contains("typewriter");
        if !is_enotty {
            return Err(format!("シリアルポートを開けません ({path}): {native_err}"));
        }
        match open_raw_tty(path, timeout) {
            Ok(f) => Ok(PortHandle::RawTty(f)),
            Err(fallback_err) => Err(format!(
                "シリアルポートを開けません ({path}): {native_err}; PTYフォールバックも失敗: {fallback_err}"
            )),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        Err(format!("シリアルポートを開けません ({path}): {native_err}"))
    }
}

/// Opens `path` directly and puts it in raw mode without setting a baud
/// rate, for macOS/iOS pseudo-terminals that reject `IOSSIOSPEED`. Mirrors
/// what `serialport::posix::tty::TTYPort::open` does, minus the baud-rate
/// ioctl. Read timeout is implemented via `VMIN`/`VTIME`.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn open_raw_tty(path: &str, timeout: Duration) -> std::io::Result<std::fs::File> {
    use std::ffi::CString;
    use std::os::fd::FromRawFd;

    let cpath = CString::new(path)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"))?;
    let fd = unsafe {
        libc::open(
            cpath.as_ptr(),
            libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    let configure = |fd: libc::c_int| -> std::io::Result<()> {
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        unsafe { libc::cfmakeraw(&mut termios) };
        termios.c_cflag |= libc::CREAD | libc::CLOCAL;
        // VMIN=0, VTIME=<timeout in deciseconds> makes reads return with
        // whatever is available (possibly nothing) after the timeout,
        // matching `serialport`'s blocking-with-timeout read behaviour.
        termios.c_cc[libc::VMIN] = 0;
        termios.c_cc[libc::VTIME] = timeout.as_millis().div_ceil(100).clamp(1, 255) as u8;
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    };

    if let Err(e) = configure(fd) {
        unsafe { libc::close(fd) };
        return Err(e);
    }
    // Clear O_NONBLOCK now that VMIN/VTIME governs read timeouts.
    unsafe { libc::fcntl(fd, libc::F_SETFL, 0) };

    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
}

/// Byte-format (main board) tag -> meaning mapping entry, as configured in
/// the UI. `tag` is a two-digit hex string, e.g. `"A1"`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteTagMapping {
    pub tag: String,
    pub kind: String,
}

/// CSV-format (valve board) column index -> meaning mapping entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvColumnMapping {
    pub index: usize,
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialStartParams {
    pub slot_id: String,
    /// Human-readable label (e.g. "メイン基板"), used only to name recording
    /// files — falls back to `slot_id` when empty.
    #[serde(default)]
    pub label: String,
    pub port_path: String,
    pub baud_rate: u32,
    /// `"byte"` (main board) or `"csv"` (valve board).
    pub format: String,
    #[serde(default)]
    pub byte_mapping: Vec<ByteTagMapping>,
    #[serde(default)]
    pub csv_mapping: Vec<CsvColumnMapping>,
    /// Byte-format frame filter (2-digit hex, e.g. `"0B"`): only frames
    /// addressed to/from these ids are decoded, everything else is dropped
    /// silently (matching the main board's own framing). Defaults to the
    /// main board's documented ids so the "main"/"valve" presets are
    /// unaffected; the UI's "custom" preset lets these be overridden for
    /// other byte-frame devices reusing the same wire format.
    #[serde(default = "default_destination_id_hex")]
    pub destination_id: String,
    #[serde(default = "default_source_id_hex")]
    pub source_id: String,
    /// Reference sea-level pressure for the pressure->altitude conversion,
    /// in the unit given by `pressure_unit`.
    #[serde(default = "default_sea_level_pressure")]
    pub sea_level_pressure_pa: f64,
    /// `"pa"`, `"hpa"`, or `"kpa"` — the unit the raw pressure value is in,
    /// for whichever format this slot is decoding (undocumented at the
    /// protocol level for the main board's TLV; the valve board's CSV
    /// documents kPa, but is also made overridable here for consistency).
    #[serde(default = "default_pressure_unit")]
    pub pressure_unit: String,
    /// When true, saves both the raw byte stream and decoded samples (CSV)
    /// to the user's Downloads folder for the lifetime of this connection.
    #[serde(default)]
    pub record: bool,
}

fn default_sea_level_pressure() -> f64 {
    101_325.0
}

fn default_pressure_unit() -> String {
    "hpa".to_string()
}

fn default_destination_id_hex() -> String {
    format!("{MAIN_DESTINATION_ID:02X}")
}

fn default_source_id_hex() -> String {
    format!("{MAIN_SOURCE_ID:02X}")
}

/// Parses a 2-digit hex byte id (optionally prefixed `0x`/`0X`), falling
/// back to `fallback` if the string is empty or invalid rather than
/// rejecting the whole connection over a malformed id field.
fn parse_hex_id(s: &str, fallback: u8) -> u8 {
    u8::from_str_radix(
        s.trim().trim_start_matches("0x").trim_start_matches("0X"),
        16,
    )
    .unwrap_or(fallback)
}

/// Multiplier to convert a raw pressure reading in `pressure_unit` to Pa.
fn pressure_unit_scale_to_pa(pressure_unit: &str) -> f64 {
    match pressure_unit {
        "kpa" => 1_000.0,
        "hpa" => 100.0,
        _ => 1.0, // "pa"
    }
}

/// Paths of the files a recording session writes to, returned to the
/// frontend so it can display where data is being saved.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingInfo {
    pub raw_path: String,
    pub csv_path: String,
}

/// Always the user's Downloads folder (falls back to home, then `.`), per
/// the current fixed-location design — no destination picker.
fn downloads_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Keeps a string filesystem-safe while preserving non-ASCII text (e.g.
/// Japanese slot labels) — `char::is_alphanumeric()` is Unicode-aware.
fn sanitize_filename_component(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "port".to_string()
    } else {
        cleaned
    }
}

/// The CSV column order for a recording session: each configured
/// (non-"ignore") data kind, in mapping order, plus the derived
/// `pressure_altitude` column when a `pressure` kind is mapped.
fn build_csv_header(kinds: &[String]) -> Vec<String> {
    let mut header: Vec<String> = Vec::new();
    for k in kinds {
        if k == "ignore" || header.contains(k) {
            continue;
        }
        header.push(k.clone());
    }
    if header.iter().any(|k| k == "pressure") && !header.iter().any(|k| k == "pressure_altitude") {
        header.push("pressure_altitude".to_string());
    }
    header
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// If `kind` is `"pressure"`, converts `n` (in the slot's configured
/// pressure unit) to a derived `pressure_altitude` value and inserts it
/// into `values`. Shared by the byte-format and CSV-format decode paths so
/// the derivation rule lives in exactly one place.
fn insert_derived_pressure_altitude(
    values: &mut HashMap<String, TelemetryValue>,
    kind: &str,
    n: f64,
    pressure_scale_to_pa: f64,
    sea_level_pressure_pa: f64,
) {
    if kind != "pressure" {
        return;
    }
    let pa = n * pressure_scale_to_pa;
    let alt = pressure_to_altitude_m(pa, sea_level_pressure_pa);
    if alt.is_finite() {
        values.insert("pressure_altitude".to_string(), TelemetryValue::Number(alt));
    }
}

fn write_csv_row(
    file: &mut File,
    header: &[String],
    recv_unix_ms: f64,
    values: &HashMap<String, TelemetryValue>,
) -> std::io::Result<()> {
    let mut fields = vec![format!("{recv_unix_ms}")];
    for k in header {
        fields.push(match values.get(k) {
            Some(TelemetryValue::Number(n)) => format!("{n}"),
            Some(TelemetryValue::Text(s)) => csv_field(s),
            None => String::new(),
        });
    }
    writeln!(file, "{}", fields.join(","))
}

/// Opens the raw (.bin) and decoded (.csv, header pre-written) recording
/// files in `downloads_dir()`, named from the slot label/id and the current
/// timestamp so repeated connections never collide.
fn open_recording_files(
    params: &SerialStartParams,
) -> Result<(File, File, Vec<String>, RecordingInfo), String> {
    let dir = downloads_dir();
    let label = sanitize_filename_component(if params.label.is_empty() {
        &params.slot_id
    } else {
        &params.label
    });
    let kinds: Vec<String> = if params.format == "byte" {
        params.byte_mapping.iter().map(|m| m.kind.clone()).collect()
    } else {
        params.csv_mapping.iter().map(|m| m.kind.clone()).collect()
    };
    let header = build_csv_header(&kinds);

    // Millisecond precision plus the slot id makes the stem unique on its
    // own; `create_new` still guards against overwriting an existing file
    // outright (e.g. a clock rollback).
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let stem = format!("telemetry_{label}_{}_{ts}", params.slot_id);
    let raw_path = dir.join(format!("{stem}_raw.bin"));
    let csv_path = dir.join(format!("{stem}.csv"));
    let raw_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&raw_path)
        .map_err(|e| {
            format!(
                "記録用ファイルを作成できません ({}): {e}",
                raw_path.display()
            )
        })?;
    let mut csv_file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&csv_path)
    {
        Ok(file) => file,
        Err(e) => {
            drop(raw_file);
            let _ = std::fs::remove_file(&raw_path);
            return Err(format!(
                "記録用ファイルを作成できません ({}): {e}",
                csv_path.display()
            ));
        }
    };

    if let Err(e) = writeln!(csv_file, "recv_unix_ms,{}", header.join(",")) {
        drop(raw_file);
        drop(csv_file);
        let _ = std::fs::remove_file(&raw_path);
        let _ = std::fs::remove_file(&csv_path);
        return Err(format!("記録用ファイルへの書き込みエラー: {e}"));
    }

    Ok((
        raw_file,
        csv_file,
        header,
        RecordingInfo {
            raw_path: raw_path.display().to_string(),
            csv_path: csv_path.display().to_string(),
        },
    ))
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum TelemetryValue {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySampleEvent {
    pub slot_id: String,
    pub format: String,
    pub recv_unix_ms: f64,
    pub values: HashMap<String, TelemetryValue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryStatusEvent {
    pub slot_id: String,
    pub good_count: u64,
    pub crc_errors: u64,
    pub length_errors: u64,
    pub unknown_tag: u64,
    pub bad_lines: u64,
    pub last_good_unix_ms: Option<f64>,
    pub disconnects: u64,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryRecordingErrorEvent {
    pub slot_id: String,
    pub message: String,
}

struct PortRuntime {
    stop: Arc<AtomicBool>,
    writer: Arc<Mutex<PortHandle>>,
    join: Option<std::thread::JoinHandle<()>>,
}

/// Tracks every currently-open serial connection, keyed by the UI's slot
/// id (not the OS port path, so a slot can be reconfigured to a new port
/// without the frontend needing to track handles itself).
#[derive(Default)]
pub struct SerialManager(Mutex<HashMap<String, PortRuntime>>);

fn now_unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0
}

fn stop_runtime(rt: PortRuntime) {
    rt.stop.store(true, Ordering::Relaxed);
    // The reader loop polls `stop` between blocking reads with a short
    // timeout, so this returns promptly.
    if let Some(handle) = rt.join {
        let _ = handle.join();
    }
}

#[tauri::command]
pub fn list_serial_ports() -> Vec<String> {
    serialport::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .unwrap_or_default()
}

#[tauri::command]
pub fn start_serial_telemetry(
    app: AppHandle,
    state: State<SerialManager>,
    params: SerialStartParams,
) -> Result<Option<RecordingInfo>, String> {
    if params.format != "byte" && params.format != "csv" {
        return Err(format!("未知のフォーマットです: {}", params.format));
    }
    // Open the port before creating any recording files: if the port fails
    // to open, nothing should be left behind in the user's Downloads folder.
    let port = open_port_handle(
        &params.port_path,
        params.baud_rate,
        Duration::from_millis(200),
    )?;

    let (recording, recording_info) = if params.record {
        match open_recording_files(&params) {
            Ok((raw_file, csv_file, header, info)) => {
                (Some((raw_file, csv_file, header)), Some(info))
            }
            Err(e) => {
                drop(port);
                return Err(e);
            }
        }
    } else {
        (None, None)
    };
    let writer_handle = port
        .try_clone()
        .map_err(|e| format!("ポート複製エラー: {e}"))?;
    let writer: Arc<Mutex<PortHandle>> = Arc::new(Mutex::new(writer_handle));
    let mut reader = port;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let slot_id = params.slot_id.clone();
    let format = params.format.clone();
    let byte_mapping = params.byte_mapping.clone();
    let csv_mapping = params.csv_mapping.clone();
    let destination_id = parse_hex_id(&params.destination_id, MAIN_DESTINATION_ID);
    let source_id = parse_hex_id(&params.source_id, MAIN_SOURCE_ID);
    // `sea_level_pressure_pa` is always in Pa (per its name/field default);
    // only the raw pressure value's unit is ambiguous and needs
    // `pressure_unit` to interpret, for either format.
    let sea_level_pressure_pa = params.sea_level_pressure_pa;
    let pressure_scale_to_pa = pressure_unit_scale_to_pa(&params.pressure_unit);
    let app_clone = app.clone();

    let join = std::thread::spawn(move || {
        run_reader_loop(ReaderLoopArgs {
            app: app_clone,
            slot_id,
            format,
            byte_mapping,
            csv_mapping,
            destination_id,
            source_id,
            sea_level_pressure_pa,
            pressure_scale_to_pa,
            reader: &mut reader,
            stop: stop_clone,
            recording,
        });
    });

    // Stopping the previous runtime for this slot (if any) blocks on
    // `JoinHandle::join()`, which must not happen while holding the lock —
    // every other slot's start/stop/send-uplink command also locks
    // `state.0`, and holding it across a blocking join would stall them for
    // as long as that reader thread takes to notice the stop flag.
    let existing = {
        let mut manager = state.0.lock().map_err(|_| "state poisoned".to_string())?;
        manager.remove(&params.slot_id)
    };
    if let Some(existing) = existing {
        stop_runtime(existing);
    }

    let mut manager = state.0.lock().map_err(|_| "state poisoned".to_string())?;
    manager.insert(
        params.slot_id,
        PortRuntime {
            stop,
            writer,
            join: Some(join),
        },
    );
    Ok(recording_info)
}

#[tauri::command]
pub fn stop_serial_telemetry(state: State<SerialManager>, slot_id: String) -> Result<(), String> {
    // See the comment in `start_serial_telemetry`: the lock must be dropped
    // before the blocking join so other slots' commands aren't stalled.
    let rt = {
        let mut manager = state.0.lock().map_err(|_| "state poisoned".to_string())?;
        manager.remove(&slot_id)
    };
    if let Some(rt) = rt {
        stop_runtime(rt);
    }
    Ok(())
}

/// Sends `text` verbatim (UTF-8 bytes, no added CR/LF) as an uplink. Per the spec's
/// warning ("文字列全体を1コマンドとして送る方式ではありません"), it is the
/// caller's responsibility to send exactly what's intended.
#[tauri::command]
pub fn send_serial_text(
    state: State<SerialManager>,
    slot_id: String,
    text: String,
) -> Result<(), String> {
    let manager = state.0.lock().map_err(|_| "state poisoned".to_string())?;
    let rt = manager
        .get(&slot_id)
        .ok_or_else(|| format!("未接続のスロットです: {slot_id}"))?;
    let mut writer = rt
        .writer
        .lock()
        .map_err(|_| "writer poisoned".to_string())?;
    writer
        .write_all(text.as_bytes())
        .map_err(|e| format!("送信エラー: {e}"))
}

struct ReaderLoopArgs<'a> {
    app: AppHandle,
    slot_id: String,
    format: String,
    byte_mapping: Vec<ByteTagMapping>,
    csv_mapping: Vec<CsvColumnMapping>,
    destination_id: u8,
    source_id: u8,
    sea_level_pressure_pa: f64,
    pressure_scale_to_pa: f64,
    reader: &'a mut PortHandle,
    stop: Arc<AtomicBool>,
    /// Raw (.bin) file, decoded (.csv) file, and the CSV's fixed column
    /// order, present only when this connection was started with `record`.
    recording: Option<(File, File, Vec<String>)>,
}

fn run_reader_loop(args: ReaderLoopArgs) {
    let ReaderLoopArgs {
        app,
        slot_id,
        format,
        byte_mapping,
        csv_mapping,
        destination_id,
        source_id,
        sea_level_pressure_pa,
        pressure_scale_to_pa,
        reader,
        stop,
        recording,
    } = args;
    let (mut raw_file, mut csv_file, csv_header) = match recording {
        Some((raw, csv, header)) => (Some(raw), Some(csv), header),
        None => (None, None, Vec::new()),
    };

    // Pre-resolve hex tag strings once instead of on every frame.
    let byte_map: Vec<(u8, String)> = byte_mapping
        .iter()
        .filter_map(|m| {
            u8::from_str_radix(m.tag.trim_start_matches("0x").trim_start_matches("0X"), 16)
                .ok()
                .map(|t| (t, m.kind.clone()))
        })
        .collect();

    let mut counters = LinkCounters::default();
    let mut frame_decoder = FrameDecoder::new();
    let mut line_buffer = ValveLineBuffer::default();
    let mut buf = [0u8; 4096];
    let mut last_status_emit = Instant::now();
    let mut connected = true;

    let emit_status = |app: &AppHandle, counters: &LinkCounters, connected: bool| {
        let _ = app.emit(
            "telemetry-status",
            TelemetryStatusEvent {
                slot_id: slot_id.clone(),
                good_count: counters.good_count,
                crc_errors: counters.crc_errors,
                length_errors: counters.length_errors,
                unknown_tag: counters.unknown_tag,
                bad_lines: counters.bad_lines,
                last_good_unix_ms: counters.last_good_unix_ms,
                disconnects: counters.disconnects,
                connected,
            },
        );
    };

    let disable_recording =
        |raw_file: &mut Option<File>, csv_file: &mut Option<File>, message: String| {
            // Treat raw and decoded output as one recording session. Once
            // either stream fails, close both so the UI cannot claim that a
            // partially written recording is still active.
            *raw_file = None;
            *csv_file = None;
            let _ = app.emit(
                "telemetry-recording-error",
                TelemetryRecordingErrorEvent {
                    slot_id: slot_id.clone(),
                    message,
                },
            );
        };

    while !stop.load(Ordering::Relaxed) {
        match reader.read(&mut buf) {
            // A zero-byte read means either "no data yet" (the raw-tty PTY
            // fallback's VMIN=0/VTIME timeout legitimately returns this) or
            // EOF (the peer closed its end). Neither case is a hard error
            // worth counting as a disconnect, but looping straight back into
            // `read()` on a fallback path that can return Ok(0) instantly
            // would busy-spin a full CPU core, so back off briefly either
            // way.
            Ok(0) => std::thread::sleep(Duration::from_millis(10)),
            Ok(n) => {
                // Preserve the raw byte stream as received, independent of
                // (and before) decoding, so a broken/updated decoder can
                // re-derive samples later (ref/NSE2026_PROTOCOL.md, "記録と
                // 時刻").
                let raw_write_error = raw_file.as_mut().and_then(|f| f.write_all(&buf[..n]).err());
                if let Some(e) = raw_write_error {
                    disable_recording(
                        &mut raw_file,
                        &mut csv_file,
                        format!("RAW記録への書き込みに失敗したため記録を停止しました: {e}"),
                    );
                }

                if format == "byte" {
                    let frames = frame_decoder.feed(&buf[..n]);
                    for err in frame_decoder.take_errors() {
                        match err {
                            FrameError::CrcMismatch => counters.crc_errors += 1,
                            FrameError::LengthMismatch | FrameError::TooShort => {
                                counters.length_errors += 1
                            }
                        }
                    }
                    for frame in frames {
                        if frame.destination_id != destination_id || frame.source_id != source_id {
                            continue;
                        }
                        match decode_main_payload(&frame.payload) {
                            Ok(result) => {
                                counters.unknown_tag += result.unknown_tags as u64;
                                let now = now_unix_ms();
                                counters.record_good(now);
                                let mut values = HashMap::new();
                                for (tag, kind) in &byte_map {
                                    let Some(decoded) = result.values.get(tag) else {
                                        continue;
                                    };
                                    let Some(n) = decoded.as_f64() else {
                                        continue;
                                    };
                                    if !n.is_finite() {
                                        continue;
                                    }
                                    values.insert(kind.clone(), TelemetryValue::Number(n));
                                    insert_derived_pressure_altitude(
                                        &mut values,
                                        kind,
                                        n,
                                        pressure_scale_to_pa,
                                        sea_level_pressure_pa,
                                    );
                                }
                                let csv_write_error = csv_file.as_mut().and_then(|f| {
                                    write_csv_row(f, &csv_header, now, &values).err()
                                });
                                if let Some(e) = csv_write_error {
                                    disable_recording(
                                        &mut raw_file,
                                        &mut csv_file,
                                        format!(
                                            "CSV記録への書き込みに失敗したため記録を停止しました: {e}"
                                        ),
                                    );
                                }
                                let _ = app.emit(
                                    "telemetry-sample",
                                    TelemetrySampleEvent {
                                        slot_id: slot_id.clone(),
                                        format: format.clone(),
                                        recv_unix_ms: now,
                                        values,
                                    },
                                );
                            }
                            Err(TlvError::Truncated)
                            | Err(TlvError::LengthMismatch { .. })
                            | Err(TlvError::DuplicateTag(_)) => {
                                counters.length_errors += 1;
                            }
                        }
                    }
                } else {
                    // CSV (valve board): documented as kPa in this format,
                    // but interpreted via `pressure_unit` like the byte
                    // format so it can be overridden per slot.
                    for line in line_buffer.feed(&buf[..n]) {
                        let cols = split_csv_columns(&line);
                        let now = now_unix_ms();
                        let mut values = HashMap::new();
                        let mut mapped_any = false;
                        for m in &csv_mapping {
                            let Some(raw) = cols.get(m.index) else {
                                continue;
                            };
                            mapped_any = true;
                            let raw = raw.trim();
                            if let Some(n) = raw.parse::<f64>().ok().filter(|n| n.is_finite()) {
                                values.insert(m.kind.clone(), TelemetryValue::Number(n));
                                insert_derived_pressure_altitude(
                                    &mut values,
                                    &m.kind,
                                    n,
                                    pressure_scale_to_pa,
                                    sea_level_pressure_pa,
                                );
                            } else {
                                values
                                    .insert(m.kind.clone(), TelemetryValue::Text(raw.to_string()));
                            }
                        }
                        if mapped_any {
                            counters.record_good(now);
                            let csv_write_error = csv_file
                                .as_mut()
                                .and_then(|f| write_csv_row(f, &csv_header, now, &values).err());
                            if let Some(e) = csv_write_error {
                                disable_recording(
                                    &mut raw_file,
                                    &mut csv_file,
                                    format!(
                                        "CSV記録への書き込みに失敗したため記録を停止しました: {e}"
                                    ),
                                );
                            }
                            let _ = app.emit(
                                "telemetry-sample",
                                TelemetrySampleEvent {
                                    slot_id: slot_id.clone(),
                                    format: format.clone(),
                                    recv_unix_ms: now,
                                    values,
                                },
                            );
                        } else {
                            counters.bad_lines += 1;
                        }
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::TimedOut => {}
            // A signal interrupting the syscall (EINTR) is not a link
            // problem — the standard response is to just retry the read,
            // not tear down the connection.
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => {
                counters.disconnects += 1;
                break;
            }
        }

        if last_status_emit.elapsed() >= Duration::from_millis(500) {
            emit_status(&app, &counters, connected);
            last_status_emit = Instant::now();
        }
    }

    // Reaching this point means the reader has stopped and its port handle is
    // about to be dropped, whether due to an explicit stop or an I/O error.
    connected = false;
    emit_status(&app, &counters, connected);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Manual smoke test for the macOS/iOS PTY fallback: opens a live
    /// NSE2026 mock port (`ref/README.md`: `uv run nse2026-mock main`) and
    /// confirms `open_port_handle` succeeds where the plain `serialport`
    /// open would fail with ENOTTY. Needs a running mock, so it's gated
    /// like `crates/simulator_core/tests/jsbsim_smoke.rs`.
    ///
    /// ```sh
    /// TELEMETRY_TEST_PORT=/dev/ttys001 cargo test -p trajecsimugui \
    ///     --lib serial::tests::opens_live_mock_port -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn opens_live_mock_port() {
        let path = std::env::var("TELEMETRY_TEST_PORT").unwrap_or_else(|_| "/dev/ttys001".into());
        let mut handle = open_port_handle(&path, 115200, Duration::from_millis(200))
            .unwrap_or_else(|e| panic!("failed to open {path}: {e}"));
        let mut buf = [0u8; 256];
        // A single read is enough to prove the port is open and readable
        // (or at least times out cleanly rather than erroring).
        match handle.read(&mut buf) {
            Ok(n) => println!("read {n} bytes from {path}"),
            Err(e) if e.kind() == ErrorKind::TimedOut => println!("read timed out (ok)"),
            Err(e) => panic!("unexpected read error: {e}"),
        }
    }
}
