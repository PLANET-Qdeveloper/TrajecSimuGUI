# 実機テレメトリ機能 (テレメトリタブ)

NSE2026 の実機テレメトリ（メイン基板のバイナリフレーム、バルブ基板のCSV）をシリアル経由で受信し、シミュレーション結果と同じ地図・グラフのUIで可視化する機能。プロトコル仕様は [ref/NSE2026_PROTOCOL.md](ref/NSE2026_PROTOCOL.md) を参照。

## 全体構成

```
┌ タブバー: パラメータ / テーブル / 結果詳細 / テレメトリ
└ テレメトリタブ
   ├ サブタブ「設定」: シリアルポート4枠 × (有効/無効, ポート選択, baud, フォーマット, データ割当)
   └ サブタブ「表示」 : GNSS 3D地図(軌跡付き) / 高度グラフ / 加速度グラフ / 電圧グラフ / 小さな数値ステータス
```

複数系統（最大4スロット）を同時接続でき、系統ごとに固定色で塗り分けて表示する（平均化はしない）。

## Backend (Rust)

### `crates/telemetry_protocol`（新規クレート）

シリアルI/Oから独立した、プロトコルのデコード/エンコードのみを担当するライブラリ。ユニットテストに仕様書のテストベクタ（CRC既知値、88byte完全フレーム例、スタッフィング例、バルブCSV例）を使用する。

| モジュール | 役割 |
|---|---|
| `crc.rs` | CRC-16/CCITT-FALSE |
| `frame.rs` | PLANET-Q共通フレームのバイトスタッフィング/デコード状態機械 (`FrameDecoder`, `encode_frame`) |
| `tlv.rs` | メイン基板ペイロードのTLVデコード (`decode_main_payload`)。Tag/Lengthを順に読み進め、未知Tagは読み飛ばして継続 |
| `valve_csv.rs` | バルブ基板CSV行を `split_csv_columns` で分割し、`ValveLineBuffer` で行バッファリング |
| `altitude.rs` | ISA対流圏の気圧高度換算 (`pressure_to_altitude_m`) |
| `counters.rs` | 受信監視カウンタ (`LinkCounters`: 正常数/CRC異常/長さ異常/未知Tag/不正行/最終正常受信時刻/切断回数) |

### `src-tauri/src/serial.rs`

- `SerialManager`（`Mutex<HashMap<slot_id, PortRuntime>>`、`tauri::State`として管理）がスロットごとの接続を保持。
- Tauriコマンド:
  - `list_serial_ports()` — OS上のシリアルポート一覧
  - `start_serial_telemetry(params)` — ポートを開き受信スレッドを起動（`params` は `slotId/portPath/baudRate/format/byteMapping/csvMapping/seaLevelPressurePa/pressureUnit` を含む単一オブジェクト）
  - `stop_serial_telemetry(slotId)` — 受信スレッド停止
  - `send_serial_text(slotId, text)` — アップリンクとして任意の文字列をそのまま生バイト列（UTF-8、CR/LF付与なし）で送信。UIの自由入力欄はこちらを使う
- イベント: `telemetry-sample`（デコード結果1件ごと）、`telemetry-status`（500ms毎にカウンタ）。どちらも `slotId` 付きペイロードでスロットを区別。
- 受信スレッド: ポートごとに専用の `std::thread::spawn`。バイトフォーマットは `FrameDecoder` → `decode_main_payload`、CSVフォーマットは `ValveLineBuffer` → `split_csv_columns` でデコードし、フロントから渡された `tag/index → 種類(DataKind文字列)` マッピングを適用して `values: HashMap<DataKind, TelemetryValue>` を組み立てて emit。

#### macOS/iOS PTYフォールバック (`PortHandle`)

`serialport` クレートは macOS/iOS では常に `IOSSIOSPEED` ioctl でボーレートを設定するが、この ioctl は疑似端末（PTY）に対しては必ず `ENOTTY`（"Not a typewriter"）で失敗する。NSE2026のモッククライアントはPTY（`/dev/ttysNNN`）で動作するため、通常の `serialport::new(...).open()` だけでは macOS からモックに接続できない。

対策として `PortHandle` enum（`Native(Box<dyn SerialPort>)` / `RawTty(std::fs::File)`）を導入し、通常オープンが失敗した場合のみ macOS/iOS限定でボーレート設定を省いた raw tty オープンにフォールバックする（`open_port_handle` / `open_raw_tty`）。PTYにはそもそも実際のボーレート概念がないため（仕様書にも明記）、フォールバックで支障はない。実機（本物のシリアルデバイス）では通常経路がそのまま使われる。

検証用の手動テスト（`#[ignore]`、`crates/simulator_core/tests/jsbsim_smoke.rs` と同じパターン）:
```sh
TELEMETRY_TEST_PORT=/dev/ttys001 cargo test -p trajecsimugui \
    --lib serial::tests::opens_live_mock_port -- --ignored --nocapture
```

## Frontend (Svelte)

### `src/lib/types/telemetry.ts`

- `DataKind` — 割当可能なデータ種別（altitude/latitude/.../accel_x/voltage/phase/num_sats/flag_can_err_valve 等）
- `defaultMainBoardByteMapping()` / `defaultValveBoardCsvMapping()` — 仕様書のTag表・CSV列構成をそのまま既定値化
- `SerialPortSlotConfig` — スロット1件分の設定（ポート/baud/フォーマット/気圧単位・海面基準気圧/マッピング）。`defaultSerialPortSlots()` でスロット1=メイン既定・スロット2=バルブ既定にし、モック環境にすぐ接続できるようにしている
- `TelemetrySample` / `TelemetryStatus` — バックエンドのイベントペイロードと1:1
- `SLOT_COLORS` — グラフ用の系統別色（Okabe-Itoパレット、色弱対応）
- `MAP_TRACK_COLORS` — 地図の軌跡・マーカー用オレンジ系パレット（空撮画像上での視認性を優先し、グラフとは独立した配色）

### `src/lib/components/telemetry/`

| コンポーネント | 役割 |
|---|---|
| `TelemetryPanel.svelte` | タブ本体。スロット状態・受信履歴・派生データ（地図トラック/グラフ系列）を保持し、設定/表示の2サブタブを切り替える |
| `SerialPortSlotEditor.svelte` | 1スロット分の設定UI（ポート選択+手入力、baud、フォーマット、プリセット、気圧単位、Tag/列番号→種類のマッピング編集） |
| `TelemetryLiveMap.svelte` | MapLibre+deck.gl。DEM地形+`pitch:60`で立体表示、`PathLayer`で軌跡・`ScatterplotLayer`で現在地マーカーを系統ごとに色分け |
| `LiveSeriesChart.svelte` | Highcharts（boostモジュール）汎用ストリーミングチャート。高度・加速度・電圧グラフから系列定義だけ渡して再利用 |
| `TelemetryStatusCards.svelte` | フェーズ・フェーズ経過時間・衛星数・CAN生存フラグの小さな数値表示、アップリンク自由入力欄（確認ダイアログ+送信履歴）。「表示」タブの左サイドバーに縦並びで配置 |

### 派生データの再計算

`history`（スロットごとの表示用受信履歴、`$state`ではないプレーンオブジェクト）は最大2,000点/スロット。上限到達時に既存点を1/2へ間引き、以後の保存間隔も2倍にするため、長時間でもセッション全体の時間範囲を維持しながらメモリ量を一定に保つ。これは表示だけの間引きであり、録画するraw/CSVファイルには全パケットが保存される。

地図/グラフ用の派生配列（`mapTracks`/`altitudeSeries`/`accelSeries`/`voltageSeries`）は `setInterval` で最大500msごとに再計算する。ただし、表示用履歴が更新された場合かつテレメトリ表示が見えている場合だけ実行する。サンプル受信の都度の描画や、設定画面・別タブにいる間の大量配列生成を避ける設計。

Highchartsへの反映は500msごとに`chart.update(..., oneToOne=true)`で最大2,000点の系列を置き換える。

## 永続化

スロット設定（`SerialPortSlotConfig[]`、4件）は既存のシミュレーション設定と同じ `app-settings.json`（`@tauri-apps/plugin-store`）に `"telemetrySlots"` キーで保存される。詳細:

- 保存: `slots` の変更を `$effect` で監視し、変わるたびに `$state.snapshot()` を取ってから丸ごと保存
- 復元: 起動時に保存件数が4件のときだけ復元。**`enabled` は復元時に強制的に `false` にリセット**し、起動直後の自動再接続はしない（安全のため、接続は毎回手動トグル）
- 受信中データ（`history`/`latest`/カウンタ）自体は保存対象外（再起動で消える、ディスクI/O肥大化防止）。ディスクへの記録が必要な場合は次節の録画機能（`record`トグル）を使う
- 保存データのマイグレーション/バージョニングは未実装（4件揃っていない場合は既定値にフォールバックするのみ）

## 受信データの保存 (Recording)

スロットごとに「受信データをDownloadsフォルダへ保存する」トグル（`SerialPortSlotConfig.record`）をONにして接続すると、接続の生存期間中、2種類のファイルを `~/Downloads`（`dirs::download_dir()`、取得できなければホーム→カレントディレクトリにフォールバック）へ書き続ける。保存先は現状固定で、ダイアログでの変更はできない。

- `telemetry_{ラベル}_{slotId}_{Unix時刻ms}_raw.bin` — デコード前の生バイト列をそのまま追記。仕様書「記録と時刻」の推奨（"受信したraw byte列を、PC受信時刻付きで保存すると、後からデコーダを改善して再解析できます"）に対応し、将来デコーダを直しても再解析できるようにする
- `telemetry_{ラベル}_{slotId}_{Unix時刻ms}.csv` — デコード成功したサンプルを1行ずつ追記。列は接続開始時のマッピング設定（`byteMapping`/`csvMapping`、"ignore"を除く）から確定し、`pressure`が含まれる場合は派生列`pressure_altitude`も追加。先頭列は`recv_unix_ms`（PC受信時刻、Unix ms）

実装は [serial.rs](src-tauri/src/serial.rs) の `open_recording_files`/`write_csv_row`。`start_serial_telemetry` コマンドは実際に書き込んだパス（`RecordingInfo { rawPath, csvPath }`）を返し、フロントは `TelemetryStatusCards.svelte` にファイル名を表示する。

制約:
- `record` は接続開始時の値のみ使用される。接続中はUIでトグル変更不可（切断後に変更）
- ラベルにファイル名として不正な文字が含まれる場合は `_` に置換（`sanitize_filename_component`、Unicode文字はそのまま保持されるので日本語ラベルもファイル名に使える）
- 書き込みエラーは無視して継続（`let _ = ...`）。ディスク容量不足などでファイルが壊れても受信自体は止めない設計

## メモリ管理

長時間受信でも無制限に増えないよう、以下で上限を設けている。

- `history`（スロットごとの表示用履歴）: 最大2,000点/スロット。上限到達ごとに既存点を半分へ間引き、将来の保存間隔を倍増する適応的ダウンサンプリング。セッション時間に比例して増えず、先頭から末尾までの概形を維持する
- `ValveLineBuffer`（Rust側の行バッファ）: 1024byte上限、改行が来ないまま超過したら破棄して再同期
- `FrameDecoder` のエラーリストは毎読み取りサイクルで即座にdrain
- `uplinkLog`（送信履歴）: 直近20件のみ保持
- Highchartsは `oneToOne=true` で更新しているため、スロット無効化時に古いseries（メモリ）も自動的に破棄される

## 既知の制約

- アップリンクにはACK/再送/通番の仕組みがない（仕様どおり、送信は確認ダイアログ+履歴のみで担保）。UIは自由入力欄のみを提供し、各byteが個別コマンドとして解釈されうる
- メイン基板の生気圧値の単位はプロトコル上未規定のため、`pressureUnit`（既定 hPa）と `seaLevelPressurePa`（常にPa）をUIで個別設定する必要がある。バルブ基板のCSV気圧列は仕様上常にkPa固定でこの設定を使わない
- 保存済みスロット設定のスキーママイグレーションは未実装

## 検証コマンド

```sh
cargo test --workspace              # telemetry_protocol含む全テスト
cargo clippy --all-targets
cargo fmt --check
pnpm run check                      # svelte-check + tsc
pnpm run fix                        # ESLint + Prettier + cargo fmt/clippy まとめて

# 手動E2E (モック使用)
uv run nse2026-mock main            # /dev/ttysNNN が main基板を模擬
uv run nse2026-mock valve           # 別の /dev/ttysNNN がバルブ基板を模擬
pnpm tauri dev
```
