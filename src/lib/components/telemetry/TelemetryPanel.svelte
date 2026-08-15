<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Store } from "@tauri-apps/plugin-store";
  import {
    type SerialPortSlotConfig,
    type TelemetrySample,
    type TelemetryStatus,
    type RecordingInfo,
    type TelemetryRecordingError,
    defaultSerialPortSlots,
    DataKind,
    numberValue,
    rawValue,
    SLOT_COLORS,
    MAP_TRACK_COLORS,
  } from "$lib/types/telemetry";
  import SerialPortSlotEditor from "./SerialPortSlotEditor.svelte";
  import TelemetryLiveMap from "./TelemetryLiveMap.svelte";
  import LiveSeriesChart, { type SeriesSpec } from "./LiveSeriesChart.svelte";
  import TelemetryStatusCards from "./TelemetryStatusCards.svelte";

  interface Props {
    visible?: boolean;
  }
  let { visible = true }: Props = $props();

  let subTab = $state<"config" | "monitor">("config");
  let slots = $state<SerialPortSlotConfig[]>(defaultSerialPortSlots());
  let portOptions = $state<string[]>([]);

  interface HistoryPoint {
    t: number;
    values: TelemetrySample["values"];
  }
  interface SlotHistory {
    points: HistoryPoint[];
    /** Store one out of every `stride` received samples. */
    stride: number;
    received: number;
  }
  // Keep the live display bounded independently of recording. When the cap
  // is reached, retain every other existing point and halve the resolution
  // of future points. This preserves the complete session time span without
  // retaining every high-rate packet or shifting a large array per sample.
  // Raw/CSV recording remains lossless in the Rust backend.
  const HISTORY_POINT_CAP = 2000;
  const history: Record<string, SlotHistory> = {};
  let historyDirty = false;
  let latest = $state<Record<string, TelemetrySample>>({});
  let status = $state<Record<string, TelemetryStatus>>({});
  let phaseSince = $state<Record<string, { phase: string; atMs: number }>>({});
  let uplinkLog = $state<Record<string, string[]>>({});
  let recordingInfo = $state<Record<string, RecordingInfo>>({});
  let nowMs = $state(Date.now());
  let sessionStartMs: number | null = null;

  let mapTracks = $state<
    {
      id: string;
      color: [number, number, number];
      points: [number, number, number][];
    }[]
  >([]);
  let altitudeSeries = $state<SeriesSpec[]>([]);
  let accelSeries = $state<SeriesSpec[]>([]);
  let voltageSeries = $state<SeriesSpec[]>([]);

  let store = $state<Store | null>(null);
  let storeReady = $state(false);

  function colorIndexOf(slotId: string): number {
    return slots.findIndex((s) => s.id === slotId);
  }

  function hexToRgb(hex: string): [number, number, number] {
    const n = parseInt(hex.slice(1), 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  }

  function retainHistorySample(sample: TelemetrySample) {
    const slotHistory =
      history[sample.slotId] ??
      (history[sample.slotId] = {
        points: [],
        stride: 1,
        received: 0,
      });
    slotHistory.received += 1;
    if ((slotHistory.received - 1) % slotHistory.stride !== 0) return;

    slotHistory.points.push({
      t: sample.recvUnixMs,
      values: sample.values,
    });
    if (slotHistory.points.length > HISTORY_POINT_CAP) {
      slotHistory.points = slotHistory.points.filter((_, i) => i % 2 === 0);
      slotHistory.stride *= 2;
    }
    historyDirty = true;
  }

  function recomputeDerived() {
    const tracks: typeof mapTracks = [];
    const altSeries: SeriesSpec[] = [];
    const accSeries: SeriesSpec[] = [];
    const voltSeries: SeriesSpec[] = [];

    for (const slot of slots) {
      if (!slot.enabled) continue;
      const slotHistory = history[slot.id];
      if (!slotHistory || slotHistory.points.length === 0) continue;
      const h = slotHistory.points;
      const idx = colorIndexOf(slot.id);
      const color = SLOT_COLORS[idx % SLOT_COLORS.length];
      const mapColor = MAP_TRACK_COLORS[idx % MAP_TRACK_COLORS.length];
      const rgb = hexToRgb(mapColor);

      const points: [number, number, number][] = [];
      const gnssAlt: [number, number][] = [];
      const pressureAlt: [number, number][] = [];
      const ax: [number, number][] = [];
      const ay: [number, number][] = [];
      const az: [number, number][] = [];
      const volt: [number, number][] = [];

      for (const p of h) {
        const lat = numberValue(p.values, DataKind.Latitude);
        const lon = numberValue(p.values, DataKind.Longitude);
        const alt = numberValue(p.values, DataKind.Altitude);
        if (lat !== undefined && lon !== undefined) {
          points.push([lon, lat, alt ?? 0]);
        }
        const t = sessionStartMs !== null ? (p.t - sessionStartMs) / 1000 : 0;
        if (alt !== undefined) gnssAlt.push([t, alt]);
        const palt = numberValue(p.values, DataKind.PressureAltitude);
        if (palt !== undefined) pressureAlt.push([t, palt]);
        const axv = numberValue(p.values, DataKind.AccelX);
        if (axv !== undefined) ax.push([t, axv]);
        const ayv = numberValue(p.values, DataKind.AccelY);
        if (ayv !== undefined) ay.push([t, ayv]);
        const azv = numberValue(p.values, DataKind.AccelZ);
        if (azv !== undefined) az.push([t, azv]);
        const v = numberValue(p.values, DataKind.Voltage);
        if (v !== undefined) volt.push([t, v]);
      }

      if (points.length > 0) tracks.push({ id: slot.id, color: rgb, points });
      if (gnssAlt.length > 0) {
        altSeries.push({
          id: `${slot.id}-gnss-alt`,
          name: `${slot.label} GNSS高度`,
          color,
          dashStyle: "Solid",
          data: gnssAlt,
        });
      }
      if (pressureAlt.length > 0) {
        altSeries.push({
          id: `${slot.id}-pressure-alt`,
          name: `${slot.label} 気圧高度`,
          color,
          dashStyle: "Dash",
          data: pressureAlt,
        });
      }
      if (ax.length > 0)
        accSeries.push({
          id: `${slot.id}-ax`,
          name: `${slot.label} X`,
          color,
          dashStyle: "Solid",
          data: ax,
        });
      if (ay.length > 0)
        accSeries.push({
          id: `${slot.id}-ay`,
          name: `${slot.label} Y`,
          color,
          dashStyle: "Dash",
          data: ay,
        });
      if (az.length > 0)
        accSeries.push({
          id: `${slot.id}-az`,
          name: `${slot.label} Z`,
          color,
          dashStyle: "Dot",
          data: az,
        });
      if (volt.length > 0)
        voltSeries.push({
          id: `${slot.id}-v`,
          name: slot.label,
          color,
          dashStyle: "Solid",
          data: volt,
        });
    }

    mapTracks = tracks;
    altitudeSeries = altSeries;
    accelSeries = accSeries;
    voltageSeries = voltSeries;
  }

  onMount(() => {
    invoke<string[]>("list_serial_ports")
      .then((p) => (portOptions = p))
      .catch(() => {});

    const unlistenSample = listen<TelemetrySample>("telemetry-sample", (e) => {
      const sample = e.payload;
      if (sessionStartMs === null) sessionStartMs = sample.recvUnixMs;
      latest = { ...latest, [sample.slotId]: sample };

      retainHistorySample(sample);

      const phaseRaw = rawValue(sample.values, DataKind.Phase);
      if (phaseRaw !== undefined) {
        const prev = phaseSince[sample.slotId];
        if (!prev || prev.phase !== phaseRaw) {
          phaseSince = {
            ...phaseSince,
            [sample.slotId]: { phase: phaseRaw, atMs: sample.recvUnixMs },
          };
        }
      }
    });

    const unlistenStatus = listen<TelemetryStatus>("telemetry-status", (e) => {
      status = { ...status, [e.payload.slotId]: e.payload };
    });

    const unlistenRecordingError = listen<TelemetryRecordingError>(
      "telemetry-recording-error",
      (e) => {
        const { slotId, message } = e.payload;
        recordingInfo = Object.fromEntries(
          Object.entries(recordingInfo).filter(([id]) => id !== slotId),
        );
        const label = slots.find((slot) => slot.id === slotId)?.label ?? slotId;
        alert(`記録エラー (${label}): ${message}`);
      },
    );

    const renderInterval = setInterval(() => {
      nowMs = Date.now();
      // Rebuilding every chart/map series is the expensive part. Do it only
      // when a display point changed and never while the monitor is hidden.
      if (historyDirty && visible && subTab === "monitor") {
        historyDirty = false;
        recomputeDerived();
      }
    }, 500);

    Store.load("app-settings.json").then(async (s) => {
      store = s;
      const savedSlots = await s.get<SerialPortSlotConfig[]>("telemetrySlots");
      if (savedSlots && savedSlots.length === 4) {
        slots = savedSlots.map((sl) => ({ ...sl, enabled: false }));
      }
      storeReady = true;
    });

    return () => {
      unlistenSample.then((f) => f());
      unlistenStatus.then((f) => f());
      unlistenRecordingError.then((f) => f());
      clearInterval(renderInterval);
      for (const slot of slots) {
        if (slot.enabled)
          invoke("stop_serial_telemetry", { slotId: slot.id }).catch(() => {});
      }
    };
  });

  $effect(() => {
    if (!storeReady || !store) return;
    store.set("telemetrySlots", $state.snapshot(slots));
    store.save();
  });

  async function connectSlot(slot: SerialPortSlotConfig) {
    try {
      const info = await invoke<RecordingInfo | null>(
        "start_serial_telemetry",
        {
          params: {
            slotId: slot.id,
            label: slot.label,
            portPath: slot.portPath,
            baudRate: slot.baudRate,
            format: slot.format,
            byteMapping: slot.byteMapping,
            csvMapping: slot.csvMapping,
            destinationId: slot.destinationId,
            sourceId: slot.sourceId,
            seaLevelPressurePa: slot.seaLevelPressurePa,
            pressureUnit: slot.pressureUnit,
            record: slot.record,
          },
        },
      );
      if (info) {
        recordingInfo = { ...recordingInfo, [slot.id]: info };
      } else if (slot.id in recordingInfo) {
        recordingInfo = Object.fromEntries(
          Object.entries(recordingInfo).filter(([id]) => id !== slot.id),
        );
      }
    } catch (e) {
      alert(`接続エラー (${slot.label}): ${e}`);
      slot.enabled = false;
    }
  }

  async function disconnectSlot(slot: SerialPortSlotConfig) {
    try {
      await invoke("stop_serial_telemetry", { slotId: slot.id });
    } catch {
      // 未接続なら無視
    }
  }

  function handleToggle(slot: SerialPortSlotConfig, enabled: boolean) {
    slot.enabled = enabled;
    historyDirty = true;
    if (enabled) connectSlot(slot);
    else disconnectSlot(slot);
  }

  async function sendUplinkText(slot: SerialPortSlotConfig, text: string) {
    if (!confirm(`${slot.label} へ「${text}」を送信しますか？`)) return;
    try {
      await invoke("send_serial_text", { slotId: slot.id, text });
      const log = uplinkLog[slot.id] ?? [];
      log.push(`${new Date().toLocaleTimeString()} ${text}`);
      uplinkLog = { ...uplinkLog, [slot.id]: log.slice(-20) };
    } catch (e) {
      alert(`送信エラー: ${e}`);
    }
  }

  const activeSlots = $derived(slots.filter((s) => s.enabled));
</script>

<div class="flex flex-col h-full overflow-hidden">
  <div class="flex border-b shrink-0 bg-gray-50">
    {#each (["config", "monitor"] as const) as tab (tab)}
      <button
        onclick={() => (subTab = tab)}
        class="px-3 py-1 text-xs font-medium border-b-2 transition-colors
               {subTab === tab
          ? 'border-primary text-primary'
          : 'border-transparent text-gray-500 hover:text-gray-700'}"
      >
        {tab === "config" ? "設定" : "表示"}
      </button>
    {/each}
  </div>

  <div class="flex-1 overflow-hidden relative">
    <div
      class="absolute inset-0 overflow-y-auto p-2 grid grid-cols-1 lg:grid-cols-2 gap-2"
      class:hidden={subTab !== "config"}
    >
      {#each slots as slot, i (slot.id)}
        <SerialPortSlotEditor
          bind:slot={slots[i]}
          colorIndex={i}
          {portOptions}
          connected={status[slot.id]?.connected ?? false}
          onToggle={(enabled) => handleToggle(slot, enabled)}
        />
      {/each}
    </div>

    <div
      class="absolute inset-0 overflow-hidden flex"
      class:hidden={subTab !== "monitor"}
    >
      <div class="w-72 shrink-0 overflow-y-auto border-r">
        <TelemetryStatusCards
          slots={activeSlots}
          {colorIndexOf}
          {latest}
          {status}
          {phaseSince}
          {nowMs}
          {uplinkLog}
          {recordingInfo}
          onSendText={sendUplinkText}
        />
      </div>
      <div
        class="flex-1 min-h-0 grid grid-cols-2 grid-rows-2 gap-px bg-gray-200"
      >
        <div class="bg-white min-h-0">
          <TelemetryLiveMap
            tracks={mapTracks}
            visible={visible && subTab === "monitor"}
          />
        </div>
        <div class="bg-white min-h-0">
          <LiveSeriesChart
            series={altitudeSeries}
            yAxisTitle="高度 (m)"
            visible={visible && subTab === "monitor"}
          />
        </div>
        <div class="bg-white min-h-0">
          <LiveSeriesChart
            series={accelSeries}
            yAxisTitle="加速度"
            visible={visible && subTab === "monitor"}
          />
        </div>
        <div class="bg-white min-h-0">
          <LiveSeriesChart
            series={voltageSeries}
            yAxisTitle="電圧 (V)"
            visible={visible && subTab === "monitor"}
          />
        </div>
      </div>
    </div>
  </div>
</div>
