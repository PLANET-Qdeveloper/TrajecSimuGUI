<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Store } from "@tauri-apps/plugin-store";
  import { message, open, save } from "@tauri-apps/plugin-dialog";
  import { checkForUpdates } from "$lib/utils/updater";

  import {
    type AppConfig,
    defaultConfig,
    type LandingAreaSummary,
    type SimSummary,
    TelemetryDataKey,
  } from "$lib/types/config";

  import SummaryMap from "$lib/components/SummaryMap.svelte";
  import DualChart from "$lib/components/DualChart.svelte";
  import KmlFileInput from "$lib/components/KmlFileInput.svelte";
  import ParamsPanel from "$lib/components/ParamsPanel.svelte";
  import RunPanel from "$lib/components/RunPanel.svelte";
  import ChartMap from "$lib/components/ChartMap.svelte";
  import Select from "$lib/components/Select.svelte";

  let activeTab = $state<"params" | "map">("params");
  let config = $state<AppConfig>(defaultConfig());
  let configFilePath = $state("");

  let running = $state(false);
  let progressMsg = $state("");
  let result = $state<SimSummary | null>(null);
  let landingAreaResult = $state<LandingAreaSummary | null>(null);
  let outDir = $state("");
  let noDem = $state(false);
  let spreadsheetUrl = $state("");
  let landingDirections = $state(8);
  let landingSpeedMax = $state(8.0);
  let landingSpeedSteps = $state(9);

  let mapLoaded = $state(false);
  let overlayKmlString = $state<string | null>(null);
  let overlayFileName = $state("");

  let showTrajectoryMarker = $state(true);
  let showBallisticCourse = $state(true);
  let showParachuteCourse = $state(true);
  let showBallisticLandingRange = $state(true);
  let showParachuteLandingRange = $state(true);
  let showImportedKmlOverlay = $state(true);

  let store: Store | null = null;
  let storeReady = $state(false);

  let selectedChartMapValue: string = $state(TelemetryDataKey.TrueAirspeedMps);

  const chartMapOptions = [
    { value: TelemetryDataKey.AltMslM, label: "Alt MSL (m)" },
    { value: TelemetryDataKey.TrueAirspeedMps, label: "Airspeed (m/s)" },
    { value: TelemetryDataKey.GroundSpeedMps, label: "Gnd Speed (m/s)" },
    { value: TelemetryDataKey.Mach, label: "Mach" },
    { value: TelemetryDataKey.QbarPa, label: "q̄ (Pa)" },
    { value: TelemetryDataKey.AlphaDeg, label: "α (°)" },
    { value: TelemetryDataKey.TotalAoaDeg, label: "AoA (°)" },
    { value: TelemetryDataKey.AxMps2, label: "Ax (m/s²)" },
    { value: TelemetryDataKey.AzMps2, label: "Az (m/s²)" },
    { value: TelemetryDataKey.ThrustN, label: "Thrust (N)" },
    { value: TelemetryDataKey.PitchDeg, label: "Pitch (°)" },
    { value: TelemetryDataKey.DownRangeM, label: "Down Range (m)" },
  ];

  onMount(() => {
    checkForUpdates().catch((e) => {
      console.error("アップデートの確認に失敗:", e);
    });
    const unlisten = listen<string>("sim-progress", (e) => {
      progressMsg = e.payload;
    });

    Store.load("app-settings.json").then(async (s) => {
      store = s;
      const savedUrl = await s.get<string>("spreadsheetUrl");
      if (savedUrl != null) spreadsheetUrl = savedUrl;
      const savedOutDir = await s.get<string>("outDir");
      if (savedOutDir != null) outDir = savedOutDir;
      const savedNoDem = await s.get<boolean>("noDem");
      if (savedNoDem != null) noDem = savedNoDem;
      const savedDirections = await s.get<number>("landingDirections");
      if (savedDirections != null) landingDirections = savedDirections;
      const savedSpeedMax = await s.get<number>("landingSpeedMax");
      if (savedSpeedMax != null) landingSpeedMax = savedSpeedMax;
      const savedSpeedSteps = await s.get<number>("landingSpeedSteps");
      if (savedSpeedSteps != null) landingSpeedSteps = savedSpeedSteps;
      const savedPath = await s.get<string>("configFilePath");
      if (savedPath) {
        try {
          config = await invoke<AppConfig>("load_config", { path: savedPath });
          configFilePath = savedPath;
        } catch {
          const savedConfig = await s.get<AppConfig>("config");
          if (savedConfig) config = savedConfig;
        }
      } else {
        const savedConfig = await s.get<AppConfig>("config");
        if (savedConfig) config = savedConfig;
      }
      const savedOverlayKml = await s.get<string | null>("overlayKmlString");
      if (savedOverlayKml != null) overlayKmlString = savedOverlayKml;
      const savedOverlayName = await s.get<string>("overlayFileName");
      if (savedOverlayName != null) overlayFileName = savedOverlayName;
      storeReady = true;
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });

  $effect(() => {
    if (!storeReady || !store) return;
    store.set("spreadsheetUrl", spreadsheetUrl);
    store.set("outDir", outDir);
    store.set("noDem", noDem);
    store.set("landingDirections", landingDirections);
    store.set("landingSpeedMax", landingSpeedMax);
    store.set("landingSpeedSteps", landingSpeedSteps);
    store.set("configFilePath", configFilePath);
    store.set("config", $state.snapshot(config));
    store.set("overlayKmlString", overlayKmlString);
    store.set("overlayFileName", overlayFileName);
    store.save();
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

  async function handleConvertLegacy() {
    const inputPath = await open({
      title: "旧形式設定ファイルを選択 (landed_area.yaml など)",
      multiple: false,
      filters: [{ name: "YAML", extensions: ["yaml", "yml"] }],
    });
    if (!inputPath) return;
    const outputDir = await open({
      title: "変換後のファイルの出力先フォルダを選択",
      directory: true,
      multiple: false,
    });
    if (!outputDir) return;
    try {
      const resultPath = await invoke<string>("convert_legacy_config", {
        inputPath: inputPath as string,
        outputDir: outputDir as string,
      });
      config = await invoke<AppConfig>("load_config", { path: resultPath });
      configFilePath = resultPath;
      await message(
        `変換が完了しました。\n出力先: ${outputDir as string}`,
        { title: "変換完了", kind: "info" },
      );
    } catch (e) {
      await message(`変換エラー:\n${e}`, { title: "変換エラー", kind: "error" });
    }
  }

  async function handleRunLandingArea() {
    await handleRunSingle(); // まずは単一シミュレーションを走らせて結果を表示してから、着陸エリア計算に進む
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
        {tab === "params" ? "パラメータ" : "結果詳細"}
      </button>
    {/each}
  </div>

  <!-- コンテンツ: タブ切り替えで SummaryMap を破棄しないよう absolute で重ねて CSS 表示切り替え -->
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
          bind:url={spreadsheetUrl}
          onsave={handleSave}
          onload={handleLoad}
          onconvertlegacy={handleConvertLegacy}
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

      <!-- 右カラム: SummaryMap + KML ファイル入力 -->
      <div class="flex flex-col overflow-hidden" style="flex: 1">
        <div class="flex-1 min-h-0">
          <SummaryMap
            kmlString={result?.kml_result ?? null}
            {overlayKmlString}
            landingAreaKmlString={landingAreaResult?.kml_result ?? null}
            visible={activeTab === "params"}
            bind:mapLoaded
            {showTrajectoryMarker}
            {showBallisticCourse}
            {showParachuteCourse}
            {showBallisticLandingRange}
            {showParachuteLandingRange}
            {showImportedKmlOverlay}
          />
        </div>
        <!-- 表示レイヤー切り替え -->
        <div
          class="flex items-center gap-3 px-2 py-1 border-t bg-gray-50 text-[10px] shrink-0 flex-wrap"
        >
          <label class="flex items-center gap-1 cursor-pointer select-none">
            <input
              class="accent-primary"
              type="checkbox"
              bind:checked={showTrajectoryMarker}
            />
            マーカー
          </label>
          <label class="flex items-center gap-1 cursor-pointer select-none">
            <input
              class="accent-primary"
              type="checkbox"
              bind:checked={showBallisticCourse}
            />
            <span class="">弾道軌跡</span>
          </label>
          <label class="flex items-center gap-1 cursor-pointer select-none">
            <input
              class="accent-primary"
              type="checkbox"
              bind:checked={showParachuteCourse}
            />
            <span class="">落下傘軌跡</span>
          </label>
          <label class="flex items-center gap-1 cursor-pointer select-none">
            <input
              class="accent-primary"
              type="checkbox"
              bind:checked={showBallisticLandingRange}
            />
            <span class="">弾道範囲</span>
          </label>
          <label class="flex items-center gap-1 cursor-pointer select-none">
            <input
              class="accent-primary"
              type="checkbox"
              bind:checked={showParachuteLandingRange}
            />
            <span class="">落下傘範囲</span>
          </label>
          <label class="flex items-center gap-1 cursor-pointer select-none">
            <input
              class="accent-primary"
              type="checkbox"
              bind:checked={showImportedKmlOverlay}
            />
            <span class="">KMLオーバーレイ</span>
          </label>
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

    <!-- result タブ -->
    <div
      class="absolute inset-0 flex overflow-hidden"
      class:hidden={activeTab !== "map"}
    >
      <!-- チャート: 3/5 幅、最大 1000px -->
      <div class="flex-[3] max-w-[1000px] min-w-0 overflow-hidden">
        <DualChart
          trajectory_ballistic={result?.trajectory_ballistic ?? null}
          trajectory_parachute={result?.trajectory_parachute ?? null}
          visible={activeTab === "map"}
        />
      </div>
      <!-- マップ: 残りを全て占有 -->
      <div class="flex-[2] min-w-0 flex flex-col overflow-hidden border-l">
        <div class="py-1.5">
          <Select
            options={chartMapOptions}
            bind:value={selectedChartMapValue}
          />
        </div>
        <div class="flex-1 min-h-0">
          <ChartMap
            latitude={result?.trajectory_ballistic.lat_deg ?? []}
            longitude={result?.trajectory_ballistic.lon_deg ?? []}
            altitude={result?.trajectory_ballistic.alt_msl_m ?? []}
            value={result
              ? (
                  result.trajectory_ballistic as unknown as Record<
                    string,
                    number[]
                  >
                )[selectedChartMapValue]
              : []}
            {overlayKmlString}
            {showImportedKmlOverlay}
          />
        </div>
      </div>
    </div>
  </div>
</div>
