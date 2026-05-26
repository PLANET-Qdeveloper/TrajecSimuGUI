# Deployment & Distribution Guide

このドキュメントは、TrajecSimuGUI の CI/CD パイプライン・自動アップデート機能の全体構成を説明します。
同じ仕組みを別の Tauri v2 プロジェクトに再現できる水準で記述します。

---

## 目次

1. [アーキテクチャ概要](#1-アーキテクチャ概要)
2. [必要な GitHub Secrets](#2-必要な-github-secrets)
3. [バージョン管理（git tag による自動伝播）](#3-バージョン管理git-tag-による自動伝播)
4. [ビルドマトリクスと成果物](#4-ビルドマトリクスと成果物)
5. [自動アップデート機構](#5-自動アップデート機構)
6. [Linux AppImage 再パッケージ](#6-linux-appimage-再パッケージ)
7. [CLI バイナリ配布](#7-cli-バイナリ配布)
8. [ライセンスファイル生成](#8-ライセンスファイル生成)
9. [セキュリティ監査](#9-セキュリティ監査)
10. [新規プロジェクトへの適用手順](#10-新規プロジェクトへの適用手順)

---

## 1. アーキテクチャ概要

```
git push --tags v1.2.3
        │
        ▼
GitHub Actions: build.yml
  ├── build job (matrix: Windows / Linux / macOS) ──> GitHub Release (draft)
  │       └── Linux: AppImage 再パッケージ + latest.json パッチ
  ├── cli job (matrix: 同上) ──────────────────────> 同じ Release に CLI バイナリ追加
  └── licenses job ────────────────────────────────> 同じ Release にライセンス HTML/TXT 追加

GitHub Actions: audit.yml (main push / PR / 毎週月曜)
  ├── cargo audit (Rust 脆弱性スキャン)
  └── pnpm audit  (JS 本番依存 脆弱性スキャン)
```

リリース後、アプリ起動時に `tauri-plugin-updater` が `latest.json` を取得し、
新バージョンがあればユーザーに通知・ダウンロード・再起動を行います。

---

## 2. 必要な GitHub Secrets

GitHub リポジトリの **Settings → Secrets and variables → Actions → Environment secrets**
（Environment 名: `deploy`）に以下を登録します。

### 必須（全プラットフォーム）

| Secret 名                             | 内容                                               |
|--------------------------------------|--------------------------------------------------|
| `TAURI_SIGNING_PRIVATE_KEY`          | minisign 秘密鍵（`tauri signer generate` で生成、改行含む全文） |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 上記鍵のパスワード                                        |
| `GITHUB_TOKEN`                       | GitHub Actions が自動付与（追加設定不要）                     |

### macOS コード署名（macOS ビルドを配布する場合）

| Secret 名                     | 内容                                                    |
|------------------------------|-------------------------------------------------------|
| `APPLE_CERTIFICATE`          | Apple Developer から取得した .p12 ファイルを base64 エンコードしたもの    |
| `APPLE_CERTIFICATE_PASSWORD` | .p12 ファイルのパスワード                                       |
| `APPLE_SIGNING_IDENTITY`     | `Developer ID Application: Your Name (TEAMID)` 形式の文字列 |
| `APPLE_ID`                   | Apple ID メールアドレス                                      |
| `APPLE_ID_PASSWORD`          | Apple ID のアプリ専用パスワード（2FA が必要）                         |
| `APPLE_TEAM_ID`              | Apple Developer Team ID（10 桁英数字）                      |

### アプリ固有（このプロジェクト）

| Secret 名               | 内容                        |
|------------------------|---------------------------|
| `GOOGLE_CLIENT_ID`     | Google OAuth クライアント ID    |
| `GOOGLE_CLIENT_SECRET` | Google OAuth クライアントシークレット |

> Google OAuth のシークレットは `src-tauri/build.rs` で `dotenvy` を通して Rust コンパイル時定数として埋め込まれます。
> ローカル開発時は `src-tauri/.env` に書けば自動読み込みされます（`.gitignore` に追加済みであることを確認）。

---

## 3. バージョン管理（git tag による自動伝播）

バージョンは **git tag が唯一の真実** です。ファイルを手動で編集する必要はありません。

### リリース手順

```bash
git tag v1.2.3
git push origin v1.2.3
```

### CI での自動書き換え

`build.yml` の `Set version from tag` ステップが以下のファイルを `sed` で書き換えます。

| ファイル                        | 書き換え内容                            |
|-----------------------------|-----------------------------------|
| `package.json`              | `"version"` フィールド                 |
| `src-tauri/tauri.conf.json` | `"version"` フィールド                 |
| `Cargo.toml`（workspace ルート） | `[workspace.package]` の `version` |
| `src-tauri/Cargo.toml`      | `[package]` の `version`           |

```bash
VERSION="${GITHUB_REF_NAME#v}"   # "v1.2.3" → "1.2.3"
sed -i.bak 's/"version": ".*"/"version": "'"$VERSION"'"/' package.json
sed -i.bak 's/"version": ".*"/"version": "'"$VERSION"'"/' src-tauri/tauri.conf.json
sed -i.bak '0,/^version = ".*"/s//version = "'"$VERSION"'"/' Cargo.toml
sed -i.bak '0,/^version = ".*"/s//version = "'"$VERSION"'"/' src-tauri/Cargo.toml
```

---

## 4. ビルドマトリクスと成果物

`build.yml` はトリガー: `push: tags: [v*]` で起動します。

### ビルド環境

| OS                    | Runner           | Rust ターゲット                 | バンドル種類     | 成果物                   |
|-----------------------|------------------|----------------------------|------------|-----------------------|
| Windows               | `windows-latest` | `x86_64-pc-windows-msvc`   | `nsis`     | `*.exe` NSIS インストーラー  |
| Linux                 | `ubuntu-22.04`   | `x86_64-unknown-linux-gnu` | `appimage` | `*.AppImage`（再パッケージ済） |
| macOS (Apple Silicon) | `macos-latest`   | `aarch64-apple-darwin`     | `app,dmg`  | `*.app` + `*.dmg`     |

### Linux システム依存関係

```bash
libwebkit2gtk-4.1-dev
libappindicator3-dev
librsvg2-dev
patchelf
libasound2-dev
libssl-dev
pkg-config
```

`awalsh128/cache-apt-pkgs-action` でキャッシュされるため、2 回目以降は高速です。

### ビルドアクション

[`tauri-apps/tauri-action@action-v0.6.2`](https://github.com/tauri-apps/tauri-action) を使用します。

```yaml
- uses: tauri-apps/tauri-action@action-v0.6.2
  with:
    args: --target ${{ matrix.target }} --bundles ${{ matrix.bundles }} --verbose
    tagName: ${{ github.ref_name }}
    releaseName: "trajecsimugui ${{ github.ref_name }}"
    releaseBody: "See the assets below to download the installer for your platform."
    releaseDraft: true
    includeUpdaterJson: true   # latest.json を生成してリリースに追加
    prerelease: false
```

`releaseDraft: true` のため、GitHub Release は最初 Draft 状態で作成されます。
3 つのプラットフォームジョブがすべて完了した後、手動で Publish してください。

---

## 5. 自動アップデート機構

### 仕組みの全体像

```
アプリ起動
  └─► tauri-plugin-updater が latest.json を取得
          URL: https://github.com/misohiyoko/TrajecSimuGUI/releases/latest/download/latest.json
  └─► 現在のバージョンと比較
  └─► 新バージョンあり → ユーザーにダイアログ表示（src/lib/utils/updater.ts）
  └─► 同意 → downloadAndInstall() → relaunch()（tauri-plugin-process）
```

### `latest.json` の構造（`includeUpdaterJson: true` で自動生成）

```json
{
  "version": "1.2.3",
  "notes": "See the assets below...",
  "pub_date": "2025-01-01T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "url": "https://github.com/.../trajecsimugui_1.2.3_x64-setup.exe",
      "signature": "<minisign署名>"
    },
    "linux-x86_64": {
      "url": "https://github.com/.../trajecsimugui_1.2.3_amd64.AppImage",
      "signature": "<minisign署名>"
    },
    "darwin-aarch64": {
      "url": "https://github.com/.../trajecsimugui_1.2.3_aarch64.dmg",
      "signature": "<minisign署名>"
    }
  }
}
```

### minisign 鍵ペアの生成

```bash
# Tauri CLI でキーペアを生成
pnpm tauri signer generate -w ~/.tauri/trajecsimugui.key

# 出力例:
# Public key: dW50cnVzdGVkIGNvbW1lbnQ6...（base64）
# Private key: <ファイルに保存>
```

- **公開鍵** → `src-tauri/tauri.conf.json` の `plugins.updater.pubkey` に設定
- **秘密鍵**（ファイル内容全体） → GitHub Secret `TAURI_SIGNING_PRIVATE_KEY` に設定
- **秘密鍵パスワード** → GitHub Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` に設定

### `tauri.conf.json` のアップデーター設定

```json
{
  "bundle": {
    "createUpdaterArtifacts": true
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/YOUR_USER/YOUR_REPO/releases/latest/download/latest.json"
      ],
      "pubkey": "<minisign公開鍵をbase64エンコードしたもの>"
    }
  }
}
```

### Rust 側のプラグイン登録（`src-tauri/Cargo.toml`）

```toml
[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
tauri-plugin-updater = "2"
tauri-plugin-process = "2"   # relaunch() に必要
```

### Rust 側のプラグイン登録（`src-tauri/src/lib.rs`）

```rust
tauri::Builder::default ()
.plugin(tauri_plugin_updater::Builder::new().build())
.plugin(tauri_plugin_process::init())   // ← relaunch() を使うために必須
.plugin(tauri_plugin_store::Builder::new().build())
.plugin(tauri_plugin_opener::init())
.plugin(tauri_plugin_dialog::init())
// ...
```

> `tauri-plugin-process` を登録しないと実行時に "process.restart not allowed. Plugin not found" エラーが発生します。

### フロントエンド側の npm パッケージ

```bash
pnpm add @tauri-apps/plugin-updater
pnpm add @tauri-apps/plugin-process
```

### アップデートチェック実装（`src/lib/utils/updater.ts`）

```typescript
import {check} from "@tauri-apps/plugin-updater";
import {relaunch} from "@tauri-apps/plugin-process";
import {ask, message} from "@tauri-apps/plugin-dialog";

export async function checkForUpdates() {
    const update = await check();
    if (!update) return;

    const agreed = await ask(`新しいバージョン (${update.version}) が利用可能です。アップデートしますか？`, {
        title: "アップデートの確認",
        kind: "info",
    });
    if (!agreed) return;

    await update.downloadAndInstall((event) => {
        // 進捗イベント: "Started" / "Progress" / "Finished"
    });

    await relaunch();
}
```

---

## 6. Linux AppImage 再パッケージ

Tauri がデフォルトで生成する AppImage には `libwebkit2gtk` などのシステムライブラリが同梱されており、
多くの Linux 環境でシステム側のライブラリと競合します。
そのため CI で AppImage を展開・除去・再パッケージします。

### 手順（`build.yml` の Repack ステップ）

```bash
# 1. AppImage を展開
"$APPIMAGE" --appimage-extract   # → squashfs-root/ に展開

# 2. 競合するシステムライブラリと hook を除去
rm -rf squashfs-root/usr/lib
rm -rf squashfs-root/apprun-hooks

# 3. 最小限の AppRun スクリプトを書き直す
cat > squashfs-root/AppRun << 'EOF'
#!/bin/bash
exec "$(dirname "$(readlink -f "$0")")/usr/bin/trajecsimugui" "$@"
EOF
chmod +x squashfs-root/AppRun
rm -f squashfs-root/AppRun.wrapped

# 4. appimagetool で再パッケージ
wget -q ".../appimagetool-x86_64.AppImage" -O appimagetool
chmod +x appimagetool
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 ./appimagetool squashfs-root "$APPIMAGE"

# 5. Tauri signer で再署名
pnpm tauri signer sign \
  --private-key "$TAURI_SIGNING_PRIVATE_KEY" \
  --password    "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" \
  "$APPIMAGE"

# 6. GitHub Release を更新し、latest.json の署名も書き換える
gh release upload $TAG "$APPIMAGE" "$APPIMAGE.sig"
jq --arg s "$(cat $APPIMAGE.sig)" \
  '.platforms["linux-x86_64"].signature = $s | .platforms["linux-x86_64-appimage"].signature = $s' \
  /tmp/latest.json > /tmp/latest_patched.json
gh release upload $TAG /tmp/latest_patched.json --clobber
```

---

## 7. CLI バイナリ配布

`cli` ジョブ（`build` ジョブ完了後に実行）が CLI バイナリをビルドして Release に追加します。

```bash
cargo build -p simulator_cli --release --target $TARGET

# ファイル名: simulator_cli_v1.2.3_x86_64-unknown-linux-gnu
cp "target/$TARGET/release/simulator_cli$EXT" "simulator_cli_${TAG}_${TARGET}${EXT}"
gh release upload "$TAG" "simulator_cli_${TAG}_${TARGET}${EXT}" --clobber
```

成果物:

| プラットフォーム | ファイル名例                                            |
|----------|---------------------------------------------------|
| Linux    | `simulator_cli_v1.2.3_x86_64-unknown-linux-gnu`   |
| Windows  | `simulator_cli_v1.2.3_x86_64-pc-windows-msvc.exe` |
| macOS    | `simulator_cli_v1.2.3_aarch64-apple-darwin`       |

---

## 8. ライセンスファイル生成

`licenses` ジョブ（`build` と `cli` の両方完了後）が生成します。

### Rust ライセンス（`cargo-about`）

```bash
cargo about generate -o third-party-licenses-rust.html about.hbs
```

`about.toml` で許可するライセンス:

```toml
# accepted = ["MIT", "Apache-2.0", "BSD-3-Clause", "MPL-2.0", ...]
```

### JS ライセンス（`license-checker-rseidelsohn`）

```bash
pnpm exec license-checker-rseidelsohn \
  --production \          # devDependencies を除外
  --plainVertical \       # 読みやすいテキスト形式
  --out third-party-licenses-js.txt
```

`package.json` でもスクリプト化されています:

```json
{
  "scripts": {
    "license:js": "license-checker-rseidelsohn --production --plainVertical --out third-party-licenses-js.txt"
  }
}
```

---

## 9. セキュリティ監査

`audit.yml` が以下のタイミングで実行されます:

- `main` ブランチへの push
- Pull Request 作成・更新
- 毎週月曜 UTC 00:00（スケジュール実行）

### Rust（`cargo audit`）

```bash
cargo audit \
  --ignore RUSTSEC-2024-0411 \   # GTK/webkit2gtk の abandonment 警告
  ... # Tauri の推移的依存で現時点では回避不能なものを ignore
  --ignore RUSTSEC-2025-0081
```

### JS（`pnpm audit`）

```bash
pnpm audit --prod --audit-level moderate
# --prod: devDependencies はビルドツールのみのため除外
```

---

## 10. 新規プロジェクトへの適用手順

同じ仕組みを別の Tauri v2 プロジェクトに適用する場合の手順です。

### ステップ 1: minisign 鍵ペアを生成

```bash
pnpm tauri signer generate -w ~/.tauri/myapp.key
# → 公開鍵（base64）と秘密鍵ファイルが生成される
```

### ステップ 2: `tauri.conf.json` を設定

```json
{
  "bundle": {
    "createUpdaterArtifacts": true
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/OWNER/REPO/releases/latest/download/latest.json"
      ],
      "pubkey": "<ステップ1で得た公開鍵>"
    }
  }
}
```

### ステップ 3: Cargo の依存関係を追加

```toml
# src-tauri/Cargo.toml
[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

### ステップ 4: Rust でプラグインを登録

```rust
// src-tauri/src/lib.rs
tauri::Builder::default ()
.plugin(tauri_plugin_updater::Builder::new().build())
.plugin(tauri_plugin_process::init())
// ...
```

### ステップ 5: npm パッケージをインストール

```bash
pnpm add @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

### ステップ 6: GitHub Secrets を登録

`Settings → Environments → deploy → Secrets` に追加:

- `TAURI_SIGNING_PRIVATE_KEY`: 秘密鍵ファイルの内容（改行含む全文）
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: 秘密鍵のパスワード
- macOS 配布する場合は Apple 関連の Secrets も追加

### ステップ 7: `.github/workflows/build.yml` を配置

このリポジトリの `build.yml` を参考に、以下を自プロジェクト向けに調整します:

- `matrix.include` の `bundles` とターゲット
- リリース名（`releaseName`）
- Linux AppImage 再パッケージが不要なら該当ステップを削除
- `GOOGLE_CLIENT_ID` など不要な Secrets を削除

### ステップ 8: 動作確認

```bash
git tag v0.1.0
git push origin v0.1.0
# → GitHub Actions が起動し、Release Draft が作成される
# → 3 ジョブすべて完了後、GitHub Releases で Draft を Publish する
```

リリース後にアプリを起動し、`checkForUpdates()` を呼び出してアップデートダイアログが表示されることを確認します。

---

## ツールバージョン（2025年時点）

| ツール                           | バージョン                               |
|-------------------------------|-------------------------------------|
| `@tauri-apps/cli`             | v2                                  |
| `tauri-apps/tauri-action`     | action-v0.6.2                       |
| Node.js                       | 22                                  |
| pnpm                          | 11.2.2                              |
| Rust                          | stable                              |
| `cargo-about`                 | latest（`taiki-e/install-action` 経由） |
| `cargo-audit`                 | latest（`taiki-e/install-action` 経由） |
| `license-checker-rseidelsohn` | ^4.4.2                              |

---

## build.yml

```yaml
name: Build & Release

on:
  push:
    tags: [ "v*" ]

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  build:
    environment: deploy
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: windows-latest
            target: x86_64-pc-windows-msvc
            bundles: nsis # exeインストーラーのみ生成
          - platform: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
            bundles: appimage # 汎用実行ファイルとDebian系インストーラーのみ生成
          - platform: macos-latest
            target: aarch64-apple-darwin
            bundles: app,dmg # M1/M2/M3用DMGのみ生成

    runs-on: ${{ matrix.platform }}
    timeout-minutes: 60
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v4

      - name: Fetch JSBSim submodule
        run: git submodule update --init jsbsim

      - name: Set version from tag
        shell: bash
        run: |
          VERSION="${GITHUB_REF_NAME#v}"
          echo "Setting version to $VERSION"
          # package.json
          sed -i.bak 's/"version": ".*"/"version": "'"$VERSION"'"/' package.json
          # tauri.conf.json
          sed -i.bak 's/"version": ".*"/"version": "'"$VERSION"'"/' src-tauri/tauri.conf.json
          # workspace Cargo.toml (workspace.package.version)
          sed -i.bak '0,/^version = ".*"/s//version = "'"$VERSION"'"/' Cargo.toml
          # src-tauri Cargo.toml
          sed -i.bak '0,/^version = ".*"/s//version = "'"$VERSION"'"/' src-tauri/Cargo.toml
          rm -f package.json.bak src-tauri/tauri.conf.json.bak Cargo.toml.bak src-tauri/Cargo.toml.bak

      - name: Install system dependencies (Linux)
        if: matrix.platform == 'ubuntu-22.04'
        uses: awalsh128/cache-apt-pkgs-action@latest
        with:
          packages: libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libasound2-dev libssl-dev pkg-config
          version: 1.0 # キャッシュのバージョン（無効化・再取得したい時は 1.1 などに変更する）

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 11.2.2

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache Rust dependencies
        uses: swatinem/rust-cache@v2
        with:
          workspaces: src-tauri -> target

      - name: Install frontend dependencies
        run: pnpm install

      - name: Verify signing secrets
        shell: bash
        env:
          K: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          P: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        run: |
          echo "key length:  ${#K}"
          echo "pass length: ${#P}"
          echo "key first byte hex: $(printf %s "$K" | head -c1 | xxd -p)"
          echo "key last  byte hex: $(printf %s "$K" | tail -c1 | xxd -p)"
          if [ -z "$K" ]; then
            echo "::error::TAURI_SIGNING_PRIVATE_KEY is empty"
            exit 1
          fi

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@action-v0.6.2
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_ID_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          TAURI_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          GOOGLE_CLIENT_ID: ${{ secrets.GOOGLE_CLIENT_ID }}
          GOOGLE_CLIENT_SECRET: ${{ secrets.GOOGLE_CLIENT_SECRET }}
        with:
          args: --target ${{ matrix.target }} --bundles ${{ matrix.bundles }} --verbose
          tagName: ${{ github.ref_name }}
          releaseName: "trajecsimugui ${{ github.ref_name }}"
          releaseBody: "See the assets below to download the installer for your platform."
          releaseDraft: true
          includeUpdaterJson: true
          prerelease: false

      - name: Repack AppImage without bundled system libs (Linux)
        if: matrix.platform == 'ubuntu-22.04'
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        run: |
          set -euo pipefail
          APPIMAGE=$(find target -name "*.AppImage" ! -name "*.tar.gz" -type f | head -1)
          FILENAME=$(basename "$APPIMAGE")
          OLD_SIG="${APPIMAGE}.sig"

          # --- Extract & strip bundled libs ---
          chmod +x "$APPIMAGE"
          "$APPIMAGE" --appimage-extract
          rm -rf squashfs-root/usr/lib
          rm -rf squashfs-root/apprun-hooks
          cat > squashfs-root/AppRun << 'EOF'
          #!/bin/bash
          exec "$(dirname "$(readlink -f "$0")")/usr/bin/trajecsimugui" "$@"
          EOF
          chmod +x squashfs-root/AppRun
          rm -f squashfs-root/AppRun.wrapped

          # --- Repack ---
          wget -q "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" -O appimagetool
          chmod +x appimagetool
          APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 ./appimagetool squashfs-root "$APPIMAGE"

          # --- Re-sign with tauri's own signer (handles password env vars correctly) ---
          rm -f "$OLD_SIG"
          pnpm tauri signer sign \
            --private-key "$TAURI_SIGNING_PRIVATE_KEY" \
            --password    "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" \
            "$APPIMAGE"
          # → これで $APPIMAGE.sig が再生成される
          ls -la "$OLD_SIG"

          # --- Upload to release ---
          gh release delete-asset ${{ github.ref_name }} "$FILENAME"     -y || true
          gh release delete-asset ${{ github.ref_name }} "$FILENAME.sig" -y || true
          gh release upload ${{ github.ref_name }} "$APPIMAGE" "$OLD_SIG"

          # --- Patch latest.json with the new sig and overwrite in-place ---
          NEW_SIG_CONTENT=$(cat "$OLD_SIG")
          gh release download ${{ github.ref_name }} -p latest.json -D /tmp/
          jq --arg s "$NEW_SIG_CONTENT" '
            .platforms["linux-x86_64"].signature = $s |
            .platforms["linux-x86_64-appimage"].signature = $s
          ' /tmp/latest.json > /tmp/latest_patched.json
          mv /tmp/latest_patched.json /tmp/latest.json
          gh release upload ${{ github.ref_name }} /tmp/latest.json --clobber


  cli:
    needs: build
    runs-on: ${{ matrix.platform }}
    timeout-minutes: 30
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
            ext: ""
          - platform: windows-latest
            target: x86_64-pc-windows-msvc
            ext: ".exe"
          - platform: macos-latest
            target: aarch64-apple-darwin
            ext: ""

    steps:
      - uses: actions/checkout@v4

      - name: Fetch JSBSim submodule
        run: git submodule update --init jsbsim

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache Rust dependencies
        uses: swatinem/rust-cache@v2
        with:
          workspaces: .

      - name: Build CLI
        run: cargo build -p simulator_cli --release --target ${{ matrix.target }}

      - name: Upload CLI to release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        shell: bash
        run: |
          BIN="target/${{ matrix.target }}/release/simulator_cli${{ matrix.ext }}"
          OUT="simulator_cli_${{ github.ref_name }}_${{ matrix.target }}${{ matrix.ext }}"
          cp "$BIN" "$OUT"
          gh release upload ${{ github.ref_name }} "$OUT" --clobber

  licenses:
    needs: [ build, cli ]
    runs-on: ubuntu-22.04
    timeout-minutes: 20
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v4

      - name: Fetch JSBSim submodule (shallow)
        run: git submodule update --init --depth 1 jsbsim

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 11.2.2

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Rust dependencies
        uses: swatinem/rust-cache@v2
        with:
          workspaces: .

      - name: Install cargo-about
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-about

      - name: Install frontend dependencies
        run: pnpm install

      - name: Generate Rust third-party licenses
        run: cargo about generate -o third-party-licenses-rust.html about.hbs

      - name: Generate JS third-party licenses
        run: |
          pnpm exec license-checker-rseidelsohn \
            --production \
            --plainVertical \
            --out third-party-licenses-js.txt

      - name: Upload license files to release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          gh release upload ${{ github.ref_name }} \
            third-party-licenses-rust.html \
            third-party-licenses-js.txt \
            --clobber

```

## audit.yml

```yaml
name: Security Audit

on:
  push:
    branches: [ main ]
  pull_request:
  schedule:
    - cron: "0 0 * * 1" # 毎週月曜 UTC 00:00

jobs:
  audit-rust:
    name: Rust (cargo audit)
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install cargo-audit
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-audit

      - name: Run cargo audit
        # GTK3 / webkit2gtk 系の abandonment 警告は Tauri 側の推移的依存であり
        # 現時点では回避不能なため --ignore で抑制する
        run: |
          cargo audit \
            --ignore RUSTSEC-2024-0411 \
            --ignore RUSTSEC-2024-0412 \
            --ignore RUSTSEC-2024-0413 \
            --ignore RUSTSEC-2024-0414 \
            --ignore RUSTSEC-2024-0415 \
            --ignore RUSTSEC-2024-0416 \
            --ignore RUSTSEC-2024-0417 \
            --ignore RUSTSEC-2024-0418 \
            --ignore RUSTSEC-2024-0419 \
            --ignore RUSTSEC-2024-0420 \
            --ignore RUSTSEC-2024-0370 \
            --ignore RUSTSEC-2025-0081

  audit-js:
    name: JS (pnpm audit)
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 11.2.2

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm

      - name: Install dependencies
        run: pnpm install

      - name: Run pnpm audit
        # devDependencies の脆弱性はビルド時ツールのみに影響するため
        # --prod でパッケージアプリに同梱される production deps のみを対象とする
        run: pnpm audit --prod --audit-level moderate

```

## updater.ts

```typescript
import {check} from "@tauri-apps/plugin-updater";
import {relaunch} from "@tauri-apps/plugin-process";
import {ask, message} from "@tauri-apps/plugin-dialog";

export async function checkForUpdates() {
    try {
        const update = await check();

        // アップデートがない場合はここで終了
        if (!update) {
            console.log("no updates available");
            return;
        }

        // 1. ユーザーにアップデートするか確認するダイアログを表示
        const userAgreed = await ask(
            `新しいバージョン (${update.version}) が利用可能です。\n\n今すぐダウンロードして再起動しますか？`,
            {
                title: "アップデートの確認",
                kind: "info",
                okLabel: "アップデートする",
                cancelLabel: "あとで",
            },
        );

        // ユーザーが「あとで」を選んだ場合は処理をキャンセル
        if (!userAgreed) {
            console.log("アップデートがキャンセルされました。");
            return;
        }

        let downloaded = 0;
        let contentLength: number | undefined = 0;

        await update.downloadAndInstall((event) => {
            switch (event.event) {
                case "Started":
                    contentLength = event.data.contentLength;
                    console.log(`ダウンロード開始: ${event.data.contentLength} bytes`);
                    break;
                case "Progress":
                    downloaded += event.data.chunkLength;
                    console.log(`ダウンロード中: ${downloaded} / ${contentLength}`);
                    // ※ ここでSvelteのストア(store)に進行状況を渡せば、画面上にプログレスバーを出すことも可能です
                    break;
                case "Finished":
                    console.log("ダウンロード完了");
                    break;
            }
        });

        // 3. 完了したら再起動する旨を伝えてから再起動
        await message("アップデートが完了しました。アプリを再起動します。", {
            title: "再起動",
            kind: "info",
            okLabel: "OK",
        });

        await relaunch();
    } catch (error) {
        console.error("アップデート処理中にエラーが発生しました:", error);
        await message("アップデートの確認中にエラーが発生しました。", {
            title: "エラー",
            kind: "error",
        });
    }
}

```

## 呼び出し例

```sveltehtml
onMount(() => {
        checkForUpdates().catch((e) => {
            console.error("アップデートの確認に失敗:", e);
        });
});
```
