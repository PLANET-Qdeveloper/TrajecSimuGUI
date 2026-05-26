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

| Secret 名 | 内容 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | minisign 秘密鍵（`tauri signer generate` で生成、改行含む全文） |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 上記鍵のパスワード |
| `GITHUB_TOKEN` | GitHub Actions が自動付与（追加設定不要） |

### macOS コード署名（macOS ビルドを配布する場合）

| Secret 名 | 内容 |
|---|---|
| `APPLE_CERTIFICATE` | Apple Developer から取得した .p12 ファイルを base64 エンコードしたもの |
| `APPLE_CERTIFICATE_PASSWORD` | .p12 ファイルのパスワード |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Your Name (TEAMID)` 形式の文字列 |
| `APPLE_ID` | Apple ID メールアドレス |
| `APPLE_ID_PASSWORD` | Apple ID のアプリ専用パスワード（2FA が必要） |
| `APPLE_TEAM_ID` | Apple Developer Team ID（10 桁英数字） |

### アプリ固有（このプロジェクト）

| Secret 名 | 内容 |
|---|---|
| `GOOGLE_CLIENT_ID` | Google OAuth クライアント ID |
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

| ファイル | 書き換え内容 |
|---|---|
| `package.json` | `"version"` フィールド |
| `src-tauri/tauri.conf.json` | `"version"` フィールド |
| `Cargo.toml`（workspace ルート） | `[workspace.package]` の `version` |
| `src-tauri/Cargo.toml` | `[package]` の `version` |

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

| OS | Runner | Rust ターゲット | バンドル種類 | 成果物 |
|---|---|---|---|---|
| Windows | `windows-latest` | `x86_64-pc-windows-msvc` | `nsis` | `*.exe` NSIS インストーラー |
| Linux | `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `appimage` | `*.AppImage`（再パッケージ済） |
| macOS (Apple Silicon) | `macos-latest` | `aarch64-apple-darwin` | `app,dmg` | `*.app` + `*.dmg` |

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
tauri::Builder::default()
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
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask, message } from "@tauri-apps/plugin-dialog";

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

| プラットフォーム | ファイル名例 |
|---|---|
| Linux | `simulator_cli_v1.2.3_x86_64-unknown-linux-gnu` |
| Windows | `simulator_cli_v1.2.3_x86_64-pc-windows-msvc.exe` |
| macOS | `simulator_cli_v1.2.3_aarch64-apple-darwin` |

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
tauri::Builder::default()
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

| ツール | バージョン |
|---|---|
| `@tauri-apps/cli` | v2 |
| `tauri-apps/tauri-action` | action-v0.6.2 |
| Node.js | 22 |
| pnpm | 11.2.2 |
| Rust | stable |
| `cargo-about` | latest（`taiki-e/install-action` 経由） |
| `cargo-audit` | latest（`taiki-e/install-action` 経由） |
| `license-checker-rseidelsohn` | ^4.4.2 |
