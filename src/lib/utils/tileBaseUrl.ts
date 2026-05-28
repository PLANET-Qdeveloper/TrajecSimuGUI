/**
 * On Windows, Tauri/WebView2 maps custom URI schemes to
 * https://{scheme}.localhost/ so that the browser Fetch API (including
 * web workers used by MapLibre) can load them as plain HTTPS URLs.
 * On macOS/Linux the native tile:// scheme works directly.
 */
export function getTileBaseUrl(): string {
  return navigator.userAgent.includes("Windows")
    ? "http://tile.localhost"
    : "tile://localhost";
}
