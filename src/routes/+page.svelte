<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open, save } from "@tauri-apps/plugin-dialog";

  import {
    defaultConfig,
    type AppConfig,
    type LandingAreaSummary,
    type SimSummary,
  } from "$lib/types/config";

  import Map from "$lib/components/Map.svelte";
  import KmlFileInput from "$lib/components/KmlFileInput.svelte";
  import ParamsPanel from "$lib/components/ParamsPanel.svelte";
  import RunPanel from "$lib/components/RunPanel.svelte";

  let activeTab = $state<"params" | "map">("params");
  let config = $state<AppConfig>(defaultConfig());
  let configFilePath = $state("");

  let running = $state(false);
  let progressMsg = $state("");
  let result = $state<SimSummary | null>(null);
  let landingAreaResult = $state<LandingAreaSummary | null>(null);
  let outDir = $state("");
  let noDem = $state(false);
  let landingDirections = $state(8);
  let landingSpeedMax = $state(8.0);
  let landingSpeedSteps = $state(9);

  let mapLoaded = $state(false);
  let overlayKmlString = $state<string | null>(null);
  let overlayFileName = $state("");

  onMount(() => {
    const unlisten = listen<string>("sim-progress", (e) => {
      progressMsg = e.payload;
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  async function handleLoad() {
    const path = await open({
      multiple: false,
      filters: [{ name: "YAML", extensions: ["yaml", "yml"] }],
    });
    if (!path) return;
    try {
      config = await invoke<AppConfig>("load_config", { path: path as string });
      configFilePath = path as string;
    } catch (e) {
      alert(`読み込みエラー: ${e}`);
    }
  }

  async function handleSave() {
    if (!configFilePath) return handleSaveAs();
    try {
      await invoke("save_config", { config, savePath: configFilePath });
    } catch (e) {
      alert(`保存エラー: ${e}`);
    }
  }

  async function handleSaveAs() {
    const path = await save({
      defaultPath: configFilePath || undefined,
      filters: [{ name: "YAML", extensions: ["yaml", "yml"] }],
    });
    if (!path) return;
    try {
      await invoke("save_config", { config, savePath: path as string });
      configFilePath = path as string;
    } catch (e) {
      alert(`保存エラー: ${e}`);
    }
  }

  async function handleImport() {
    const path = await open({
      multiple: false,
      filters: [{ name: "YAML", extensions: ["yaml", "yml"] }],
    });
    if (!path) return;
    try {
      config = await invoke<AppConfig>("load_config", { path: path as string });
      // configFilePath はそのまま（インポートは現在ファイルを変えない）
    } catch (e) {
      alert(`インポートエラー: ${e}`);
    }
  }

  async function handleRunLandingArea() {
    running = true;
    landingAreaResult = null;
    progressMsg = "";
    try {
      landingAreaResult = await invoke<LandingAreaSummary>("run_landing_area", {
        config,
        outDir,
        noDem,
        directions: landingDirections,
        speedMax: landingSpeedMax,
        speedSteps: landingSpeedSteps,
      });
    } catch (e) {
      progressMsg = `エラー: ${e}`;
    } finally {
      running = false;
    }
  }

  async function handleRunSingle() {
    running = true;
    result = null;
    progressMsg = "";
    try {
      result = await invoke<SimSummary>("run_simulation", {
        config,
        outDir,
        noDem,
      });
    } catch (e) {
      progressMsg = `エラー: ${e}`;
    } finally {
      running = false;
    }
  }
</script>

<div class="flex flex-col h-screen overflow-hidden bg-white text-black">
  <!-- タブバー -->
  <div class="flex border-b shrink-0">
    {#each ["params", "map"] as const as tab (tab)}
      <button
        onclick={() => (activeTab = tab)}
        class="px-4 py-1.5 text-xs font-medium border-b-2 transition-colors
               {activeTab === tab
          ? 'border-primary text-primary'
          : 'border-transparent text-gray-500 hover:text-gray-700'}"
      >
        {tab === "params" ? "パラメータ" : "マップ"}
      </button>
    {/each}
  </div>

  <!-- コンテンツ: タブ切り替えで Map を破棄しないよう absolute で重ねて CSS 表示切り替え -->
  <div class="flex-1 overflow-hidden relative">
    <!-- params タブ -->
    <div
      class="absolute inset-0 flex overflow-hidden"
      class:hidden={activeTab !== "params"}
    >
      <!-- 左カラム: パラメータ + 実行パネル -->
      <div class="flex flex-col w-120 min-w-120 border-r overflow-hidden">
        <ParamsPanel
          bind:config
          {configFilePath}
          class="flex-1 overflow-hidden"
          onsave={handleSave}
          onload={handleLoad}
          onimport={handleImport}
        />
        <RunPanel
          bind:running
          bind:progressMsg
          bind:result
          bind:landingAreaResult
          bind:outDir
          bind:noDem
          bind:landingDirections
          bind:landingSpeedMax
          bind:landingSpeedSteps
          class="border-t shrink-0"
          on_run_single={handleRunSingle}
          on_run_parallel={handleRunLandingArea}
        />
      </div>

      <!-- 右カラム: Map + KML ファイル入力 -->
      <div class="flex flex-col overflow-hidden" style="flex: 1">
        <div class="flex-1 min-h-0">
          <Map
            kmlString={result?.kml_result ?? null}
            {overlayKmlString}
            landingAreaKmlString={landingAreaResult?.kml_result ?? null}
            visible={activeTab === "params"}
            bind:mapLoaded
          />
        </div>
        <KmlFileInput
          filename={overlayFileName}
          onload={(kml, name) => {
            overlayKmlString = kml;
            overlayFileName = name;
          }}
          onclear={() => {
            overlayKmlString = null;
            overlayFileName = "";
          }}
        />
      </div>
    </div>

    <!-- map タブ -->
    <div class="absolute inset-0" class:hidden={activeTab !== "map"}>
      <div class="p-4 text-sm text-gray-500">TODO: 詳細な結果画面</div>
    </div>
  </div>
</div>
