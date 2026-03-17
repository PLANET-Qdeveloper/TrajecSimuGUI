# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

**Development (frontend only):**
```
pnpm dev
```

**Development (full Tauri app):**
```
pnpm tauri dev
```

**Build frontend:**
```
pnpm build
```

**Build Tauri app:**
```
pnpm tauri build
```

**Type-check Svelte:**
```
pnpm check
```

**Package manager:** pnpm (not npm or yarn)

## Architecture

This is a **Tauri v2** desktop app using **SvelteKit** (SPA mode) + **TypeScript** on the frontend and **Rust** on the backend.

### Key constraints
- SvelteKit runs as a static SPA (no SSR) — `ssr = false` in `src/routes/+layout.ts`, adapter-static with `fallback: "index.html"`.
- Tauri's dev server expects the Vite frontend at `http://localhost:1420`.
- Frontend is built to `../build` (relative to `src-tauri/`).

### Frontend (`src/`)
- **Svelte 5** with runes (`$state`, etc.)
- **Tailwind CSS v4** (via `@tailwindcss/vite` plugin — no `tailwind.config.js` needed)
- **MapLibre GL** for map rendering with local raster tiles served from `public/tiles/{z}/{x}/{y}.jpg`
- Map is centered on the Fukuoka area (130.4, 33.6) and uses locally hosted tiles at zoom level 10
- `src/lib/components/Map.svelte` — MapLibre map component; loads local aerial tiles, renders GeoJSON route lines

### Backend (`src-tauri/`)
- Rust entry: `src-tauri/src/lib.rs` (logic) + `src-tauri/src/main.rs` (binary entry)
- Tauri commands are registered in `lib.rs` via `invoke_handler` and called from the frontend with `invoke()` from `@tauri-apps/api/core`
- Currently has a sample `greet` command

### Adding Tauri commands
1. Define `#[tauri::command]` fn in `src-tauri/src/lib.rs`
2. Register it in `tauri::generate_handler![...]`
3. Call from frontend: `invoke("command_name", { args })` (returns a Promise)
