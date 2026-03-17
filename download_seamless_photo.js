
// download_tiles.js
import fs from 'fs';
import https from 'https';
import path from 'path';

// --- 設定 ---
const ZOOM_LEVELS = [7, 8, 9, 10]; // ダウンロードするズームレベルの一覧
// ダウンロードしたい範囲（例：福岡市周辺）の緯度経度
const LAT_MIN = 24.0;
const LAT_MAX = 45.5;
const LON_MIN = 122.9;
const LON_MAX = 153.9;

// 国土地理院 シームレス写真（全国最新）のURLテンプレート
const TILE_URL_BASE = "https://cyberjapandata.gsi.go.jp/xyz/seamlessphoto";
const OUTPUT_DIR = "./public/tiles"; // Svelteの公開フォルダ内に保存

// 待機時間 (ミリ秒) - サーバー負荷軽減のため必須
const DELAY_MS = 500; 
// -----------

// 緯度経度からタイル座標(X, Y)を計算する関数
function lon2tile(lon, zoom) {
  return Math.floor(((lon + 180) / 360) * Math.pow(2, zoom));
}
function lat2tile(lat, zoom) {
  return Math.floor(
    ((1 - Math.log(Math.tan((lat * Math.PI) / 180) + 1 / Math.cos((lat * Math.PI) / 180)) / Math.PI) / 2) * Math.pow(2, zoom)
  );
}

// 画像をダウンロードして保存する関数
function downloadImage(url, dest) {
  return new Promise((resolve, reject) => {
    https.get(url, (res) => {
      if (res.statusCode !== 200) {
        reject(new Error(`Status Code: ${res.statusCode} (${url})`));
        return;
      }
      const file = fs.createWriteStream(dest);
      res.pipe(file);
      file.on('finish', () => {
        file.close();
        resolve();
      });
    }).on('error', (err) => {
      fs.unlink(dest, () => reject(err));
    });
  });
}

// 待機用関数
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function downloadZoom(zoom) {
  const xMin = lon2tile(LON_MIN, zoom);
  const xMax = lon2tile(LON_MAX, zoom);
  const yMin = lat2tile(LAT_MAX, zoom); // 緯度は北(MAX)がY座標の最小値になる
  const yMax = lat2tile(LAT_MIN, zoom);

  console.log(`\n[Zoom ${zoom}] X: ${xMin}〜${xMax}, Y: ${yMin}〜${yMax}`);

  let downloaded = 0;
  let skipped = 0;
  const total = (xMax - xMin + 1) * (yMax - yMin + 1);

  for (let x = xMin; x <= xMax; x++) {
    for (let y = yMin; y <= yMax; y++) {
      const dirPath = path.join(OUTPUT_DIR, zoom.toString(), x.toString());
      if (!fs.existsSync(dirPath)) {
        fs.mkdirSync(dirPath, { recursive: true });
      }

      const fileName = `${y}.jpg`;
      const filePath = path.join(dirPath, fileName);
      const url = `${TILE_URL_BASE}/${zoom}/${x}/${y}.jpg`;

      if (!fs.existsSync(filePath)) {
        downloaded++;
        process.stdout.write(`\r[Zoom ${zoom}] ${downloaded + skipped}/${total} ダウンロード中: ${x}/${y}   `);
        try {
          await downloadImage(url, filePath);
          await sleep(DELAY_MS); // サーバーへの配慮
        } catch (error) {
          console.error(`\n失敗: ${error.message}`);
        }
      } else {
        skipped++;
      }
    }
  }
  console.log(`\n[Zoom ${zoom}] 完了 (新規: ${downloaded}, スキップ: ${skipped})`);
}

async function main() {
  for (const zoom of ZOOM_LEVELS) {
    await downloadZoom(zoom);
  }
  console.log("\n全ズームレベルのダウンロード完了！");
}

main();