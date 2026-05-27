# TrajecSimuGUI

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![GitHub Release](https://img.shields.io/github/v/release/misohiyoko/TrajecSimuGUI)](https://github.com/misohiyoko/TrajecSimuGUI/releases/latest)

ロケットの弾道シミュレーションを実行する GUI アプリです。
パラメータ入力・シミュレーション実行・軌跡の可視化をウィンドウ上でまとめて行えます。

---

## ダウンロード・インストール

[**GitHub Releases**](https://github.com/misohiyoko/TrajecSimuGUI/releases/latest) から
お使いの OS に合ったファイルをダウンロードしてください。

| OS | ファイル | 形式 |
|---|---|---|
| Windows | `trajecsimugui_x.x.x_x64-setup.exe` | NSIS インストーラー |
| macOS (Apple Silicon) | `trajecsimugui_x.x.x_aarch64.dmg` | ディスクイメージ |
| Linux | `trajecsimugui_x.x.x_amd64.AppImage` | AppImage |

### Windows

インストーラー（.exe）を実行してください。

**SmartScreen 警告への対応**

署名なしのアプリとして扱われ、以下のような警告が表示される場合があります。

<!-- TODO: SmartScreen 警告画面のスクリーンショットをここに追加 -->
> 「Windows によって PC が保護されました」

「詳細情報」をクリックすると「実行」ボタンが現れます。そちらをクリックしてインストールを続行してください。

<!-- TODO: 「詳細情報」→「実行」の操作画面スクリーンショット -->

### macOS

.dmg を開き、アプリをアプリケーションフォルダへドラッグしてください。

**Gatekeeper 警告への対応**

初回起動時に「開発元を確認できないため開けません」と表示された場合:

1. Finder でアプリを **右クリック**（または Control + クリック）
2. 「開く」を選択
3. 確認ダイアログで「開く」をクリック

または システム設定 → プライバシーとセキュリティ → 「このまま開く」から許可することもできます。

### Linux

```bash
chmod +x trajecsimugui_x.x.x_amd64.AppImage
./trajecsimugui_x.x.x_amd64.AppImage
```

FUSE が使用できない環境では以下の方法でも実行できます。

```bash
./trajecsimugui_x.x.x_amd64.AppImage --appimage-extract-and-run
```

---

## GUI 概要

### アプリ全体

<!-- TODO: アプリ全体のスクリーンショットを追加 -->

アプリは主に3つの領域で構成されています。

- **左ペイン（パラメータ入力）**: ロケットのスペックや打ち上げ条件を入力します
- **中央（実行・結果）**: シミュレーションを実行し、結果サマリーを確認します
- **右ペイン（可視化）**: 軌跡マップ・グラフを表示します

### パラメータ入力パネル

<!-- TODO: パラメータ入力パネルのスクリーンショットを追加 -->

config.yaml ファイルを読み込むか、各フィールドを直接入力します。
Google スプレッドシート連携ボタンからスプレッドシートの値を一括反映することもできます。

### シミュレーション実行・結果パネル

<!-- TODO: 実行パネルのスクリーンショットを追加 -->

「シミュレーション実行」ボタンを押すと計算が始まります。完了後、以下が表示されます。

- 頂点高度、最大速度、飛行時間
- 落下地点（緯度・経度）
- 出力ファイルのパス（CSV、KML）

### 軌跡マップ・グラフ

<!-- TODO: 軌跡マップとグラフのスクリーンショットを追加 -->

実行結果の軌跡を地図上に重ねて表示します。高度・速度・加速度等のグラフも確認できます。

---

## 基本的な使い方

### 1. 設定ファイルを用意する

事前に [config.yaml](#入力パラメータの書式-configyaml) を作成しておくか、
GUI 上のフォームに直接入力します。

### 2. 設定ファイルを読み込む

パラメータ入力パネルの「ファイルを開く」ボタンから config.yaml を選択します。
関連する CSV テーブルは config.yaml と同じディレクトリ（または指定パス）に置いてください。

### 3. パラメータを確認・調整する

読み込んだ値がフォームに反映されます。必要に応じて各フィールドを変更してください。

### 4. シミュレーションを実行する

「シミュレーション実行」ボタンをクリックします。
計算が完了すると結果サマリーが表示され、出力ファイルが保存されます。

### 5. 結果を確認する

- **軌跡マップ**: 地図上で落下点・軌跡を視覚的に確認
- **グラフ**: 高度、速度、加速度、迎角などの時系列グラフ
- **CSV** (`mainline.csv`, `parachute.csv`): 全ステップのデータ
- **KML** (`trajectory.kml`): Google Earth で開ける軌跡ファイル
- **サマリー** (`summary.json`): 頂点、最大速度、着地点などの要約

---

## 入力パラメータの書式（config.yaml）

### サンプル config.yaml

```yaml
launch:
  latitude: 35.0
  longitude: 139.0
  elevation: 5.0
  rail_length: 5.0
  pitch: 89.0
  roll: 0.0
  yaw: 0.0
  wind_speed_mps: 3.0
  wind_direction_deg: 270.0
  wind_reference_alt: 10.0
  wind_power_exponent: 0.1667

body:
  diameter: 0.15
  dry_mass_with_fuel_section: 28.0
  cg: [1.0, 0.0, 0.0]
  inertia: [15.0, 15.0, 0.2, 0.0, 0.0, 0.0]

engine:
  thrust_table: tables/thrust.csv
  thruster_pos: [2.0, 0.0, 0.0]
  tank:
    position: [0.8, 0.0, 0.0]
    tank_contents: 2.0
  fuel:
    position: [0.8, 0.0, 0.0]
    fuel_section_weight: 1.5
    fuel_section_weight_after_burn: 0.1

aero:
  cp_at_launch: [1.2, 0.0, 0.0]
  cp_mach_table: tables/cp_mach.csv
  cd0_alpha_mach_table: tables/cd_alpha_mach.csv
  cn_table: tables/cn_mach.csv
  cs_table: tables/cs_mach.csv
  roll_damping: 0.0
  pitch_damping: 0.0
  yaw_damping: 0.0

parachute:                          # 省略可
  terminal_velocity_table: tables/vterm.csv
  deploy_delay_sec: 1.0

sim:
  flight_duration: 120.0
  time_step: 0.01
  csv_sample_interval: 1
  kml_sample_interval: 10
```

CSV テーブルのパスは config.yaml からの相対パスで記述します。

---

### `launch` セクション

| パラメータ | 型 | 単位 | 説明 |
|---|---|---|---|
| `latitude` | float | deg | 発射地点の緯度 |
| `longitude` | float | deg | 発射地点の経度 |
| `elevation` | float | m | 発射台の海抜高度（MSL） |
| `rail_length` | float | m | 発射レールの長さ |
| `pitch` | float | deg | ピッチ角（90 = 垂直打ち上げ） |
| `roll` | float | deg | ロール角 |
| `yaw` | float | deg | ヨー角 |
| `wind_speed_mps` | float | m/s | 基準高度での風速 |
| `wind_direction_deg` | float | deg | 風向（気象慣例：風が吹いてくる方向、0 = 北風） |
| `wind_reference_alt` | float | m | 風速プロファイルの基準高度 |
| `wind_power_exponent` | float | — | べき乗則の指数の分母（開けた地形 ≈ 6） |
| `wind_table` | string | — | （オプション）高度別風速テーブルの CSV パス。指定時は上記スカラー風値より優先 |

> **風速プロファイル**: `V(h) = wind_speed_mps × (h / wind_reference_alt) ^ wind_power_exponent`

---

### `body` セクション

| パラメータ | 型 | 単位 | 説明 |
|---|---|---|---|
| `diameter` | float | m | 機体直径 |
| `dry_mass_with_fuel_section` | float | kg | 燃料タンク空・燃料セクション込みの乾燥質量 |
| `cg` | [x, y, z] | m | 重心位置（機体軸座標、ノーズ先端を原点） |
| `inertia` | [Ixx, Iyy, Izz, Ixy, Ixz, Iyz] | kg·m² | 慣性テンソルの6成分 |

---

### `engine` セクション

| パラメータ | 型 | 単位 | 説明 |
|---|---|---|---|
| `thrust_table` | string | — | 推力テーブル CSV のパス |
| `thruster_pos` | [x, y, z] | m | スラスター位置 |

#### `engine.tank` サブセクション

| パラメータ | 型 | 単位 | 説明 |
|---|---|---|---|
| `position` | [x, y, z] | m | タンク重心位置 |
| `tank_contents` | float | kg | 酸化剤・推進剤の充填質量 |
| `drain_position` | [x, y, z] | m | （オプション）排出口位置 |

#### `engine.fuel` サブセクション

| パラメータ | 型 | 単位 | 説明 |
|---|---|---|---|
| `position` | [x, y, z] | m | 燃料セクションの重心位置 |
| `fuel_section_weight` | float | kg | 燃焼前の燃料質量 |
| `fuel_section_weight_after_burn` | float | kg | 燃焼後の残存質量（容器等） |

---

### `aero` セクション

| パラメータ | 型 | 単位 | 説明 |
|---|---|---|---|
| `cp_at_launch` | [x, y, z] | m | 打ち上げ時の風圧中心位置 |
| `cp_mach_table` | string | — | マッハ数に対する CP 位置テーブル CSV のパス |
| `cd0_alpha_mach_table` | string | — | 迎角×マッハ数に対する CD0 テーブル CSV のパス |
| `cn_table` | string | — | 法線力係数テーブル CSV のパス |
| `cs_table` | string | — | 横力係数テーブル CSV のパス |
| `roll_damping` | float | — | ロール減衰係数 |
| `pitch_damping` | float | — | ピッチ減衰係数 |
| `yaw_damping` | float | — | ヨー減衰係数 |

---

### `parachute` セクション（省略可）

`parachute` セクションを省略するとパラシュートなしで計算します。

| パラメータ | 型 | 単位 | 説明 |
|---|---|---|---|
| `terminal_velocity_table` | string | — | 終端速度テーブル CSV のパス |
| `deploy_delay_sec` | float | s | 頂点検出後の展開遅延時間 |

---

### `sim` セクション

| パラメータ | 型 | 単位 | 説明 |
|---|---|---|---|
| `flight_duration` | float | s | シミュレーション最大時間 |
| `time_step` | float | s | 積分タイムステップ |
| `csv_sample_interval` | int | ステップ数 | CSV へ出力する間引き間隔 |
| `kml_sample_interval` | int | ステップ数 | KML へ出力する間引き間隔 |

---

### CSV テーブルのフォーマット

#### 推力テーブル（`thrust_table`）

```csv
time_sec,thrust_n
0.0,1200.0
0.5,1350.0
2.0,1100.0
3.0,0.0
```

1 列目: 燃焼開始からの経過時間 [s]、2 列目: 推力 [N]

#### 風圧中心テーブル（`cp_mach_table`）

```csv
mach,cp_x_m
0.0,1.20
0.5,1.18
1.0,1.15
```

1 列目: マッハ数、2 列目: CP の軸方向位置 [m]

#### CD0 テーブル（`cd0_alpha_mach_table`）

迎角（行）× マッハ数（列）の 2 次元テーブルです。

```csv
alpha_deg\mach,0.0,0.5,1.0,1.5
0.0,0.40,0.42,0.55,0.50
5.0,0.45,0.47,0.60,0.55
10.0,0.55,0.57,0.70,0.65
```

1 行目がヘッダー（マッハ数）、1 列目が迎角 [deg]、残りがその条件での CD0 値。
マッハ数は 2 点以上必要です。

#### 法線力・横力係数テーブル（`cn_table` / `cs_table`）

```csv
mach,cn
0.0,3.5
0.5,3.6
1.0,4.0
```

1 列目: マッハ数、2 列目: 係数値

#### 終端速度テーブル（`terminal_velocity_table`）

```csv
t_since_deploy_sec,v_terminal_mps
0.0,8.0
9999.0,8.0
```

1 列目: パラシュート展開後の経過時間 [s]、2 列目: 標準海面密度での終端速度 [m/s]。
高度補正は自動で行われます（`v_actual = v_SL × √(ρ₀/ρ)`）。

---

## 旧データの移行（Python TrajecSimu 形式）

以前の Python 版 TrajecSimu で作成した設定ファイルは、フィールド名や構造が異なります。
GUI の「旧形式からの変換」機能（またはCLI の `convert-legacy` コマンド）で自動変換できます。

### GUI での変換手順

<!-- TODO: 旧形式変換ダイアログのスクリーンショットを追加 -->

1. メニューまたはボタンから「旧形式の設定ファイルを変換」を選択
2. 旧 YAML ファイルを選択
3. 出力先フォルダを指定
4. 「変換」を実行 → `config.yaml` と `tables/` が生成されます

### フィールドマッピング表

| 旧形式フィールド | 新形式フィールド | 備考 |
|---|---|---|
| `launch.launcher_length` | `launch.rail_length` | |
| `launch.pitch` (リスト) | `launch.pitch` (スカラー) | リストの 1 要素目を使用 |
| `launch.yaw` (リスト) | `launch.yaw` (スカラー) | リストの 1 要素目を使用 |
| `launch.ground_wind_speed` (リスト) | `launch.wind_speed_mps` | リストの 1 要素目を使用 |
| `launch.ground_wind_dir` (リスト) | `launch.wind_direction_deg` | リストの 1 要素目を使用 |
| `launch.wind_ref_altitude` | `launch.wind_reference_alt` | |
| `launch.wind_power_factor` | `launch.wind_power_exponent` | デフォルト 0.16666 |
| `rocket.dry_weight` | `body.dry_mass_with_fuel_section` | |
| `rocket.cg_x/y/z` | `body.cg: [x, y, z]` | |
| `rocket.cp_x/y/z` | `aero.cp_at_launch: [x, y, z]` | |
| `rocket.inertia_xx/yy/zz/xy/xz/yz` | `body.inertia: [Ixx,Iyy,Izz,Ixy,Ixz,Iyz]` | |
| `rocket.tank_x/y/z` | `engine.tank.position: [x, y, z]` | |
| `rocket.tank_capacity` | `engine.tank.tank_contents` | |
| `rocket.fuel_x/y/z` | `engine.fuel.position: [x, y, z]` | |
| `rocket.fuel_capacity` | `engine.fuel.fuel_section_weight` | |
| `rocket.fuel_after_burn` | `engine.fuel.fuel_section_weight_after_burn` | |
| `rocket.thruster_x/y/z` | `engine.thruster_pos: [x, y, z]` | |
| `rocket.terminal_velocity` (リスト) | `parachute.terminal_velocity_table` | 変換時に CSV ファイルを自動生成 |
| `rocket.cd0_table` + `cdmach_table` | `aero.cd0_alpha_mach_table` | 2 つのテーブルを統合した 2D CSV に変換 |
| `simulation.parachute_deploy_delay` | `parachute.deploy_delay_sec` | |
| `simulation.output_rate` | `sim.csv_sample_interval` | `kml_sample_interval` = output_rate × 10 |

---

## 開発者向けビルド手順

### 前提条件

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 22+
- [pnpm](https://pnpm.io/) 11+

```bash
# リポジトリをクローン（サブモジュール含む）
git clone --recursive https://github.com/misohiyoko/TrajecSimuGUI.git
cd TrajecSimuGUI

# フロントエンド依存関係のインストール
pnpm install

# 開発サーバー起動
pnpm tauri dev

# リリースビルド
pnpm tauri build
```

### CLI の使い方

```bash
# シミュレーション実行
cargo run -p simulator_cli -- run -c path/to/config.yaml --out-dir out/

# 設定ファイルの検証
cargo run -p simulator_cli -- validate -c path/to/config.yaml

# パラメータの確認
cargo run -p simulator_cli -- inspect -c path/to/config.yaml

# 旧形式からの変換
cargo run -p simulator_cli -- convert-legacy -i old_config.yaml -o output_dir/

# 着地範囲の風向スイープ
cargo run -p simulator_cli -- landing-area -c path/to/config.yaml
```

---

## ライセンス

### 本プロジェクトのコード

本リポジトリに含まれる **自作コード**（`src/`、`src-tauri/`、`crates/` 以下）は
[MIT ライセンス](LICENSE-MIT) または [Apache License 2.0](LICENSE-APACHE) のデュアルライセンスです。
いずれかをご自由にお選びください。

### サードパーティライセンスに関する注意

本アプリはチャート描画に **[Highcharts](https://www.highcharts.com/)** を使用しています。
Highcharts は [Highsoft Standard License](https://www.highcharts.com/license) のもとで提供される商用ライブラリです。

- **無償利用可能な範囲**: 非営利・個人・教育目的のみ
- **商用利用・商用再配布**: Highsoft 社からの有償ライセンス取得が必要です

したがって、本アプリを **商用目的で利用・再配布する場合は Highcharts の商用ライセンスを別途取得してください**。
MIT/Apache-2.0 の適用範囲は本プロジェクト自身のコードに限定されており、Highcharts ライブラリには及びません。

その他の依存ライブラリのライセンス一覧は、リリースページの `third-party-licenses-rust.html` および `third-party-licenses-js.txt` をご参照ください。
