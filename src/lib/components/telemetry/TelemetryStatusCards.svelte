<script lang="ts">
  import {
    type SerialPortSlotConfig,
    type TelemetrySample,
    type TelemetryStatus,
    type RecordingInfo,
    DataKind,
    numberValue,
    rawValue,
    slotColor,
  } from "$lib/types/telemetry";
  import Input from "$lib/components/Input.svelte";

  interface Props {
    slots: SerialPortSlotConfig[];
    colorIndexOf: (slotId: string) => number;
    latest: Record<string, TelemetrySample>;
    status: Record<string, TelemetryStatus>;
    phaseSince: Record<string, { phase: string; atMs: number }>;
    nowMs: number;
    uplinkLog: Record<string, string[]>;
    recordingInfo: Record<string, RecordingInfo>;
    onSendText: (slot: SerialPortSlotConfig, text: string) => void;
  }

  let {
    slots,
    colorIndexOf,
    latest,
    status,
    phaseSince,
    nowMs,
    uplinkLog,
    recordingInfo,
    onSendText,
  }: Props = $props();

  let drafts = $state<Record<string, string>>({});

  function formatElapsed(ms: number): string {
    if (ms < 0 || !Number.isFinite(ms)) return "--:--";
    const totalSec = Math.floor(ms / 1000);
    const m = Math.floor(totalSec / 60);
    const sec = totalSec % 60;
    return `${m}:${sec.toString().padStart(2, "0")}`;
  }

  function send(slot: SerialPortSlotConfig) {
    const text = drafts[slot.id];
    if (!text) return;
    onSendText(slot, text);
    drafts = { ...drafts, [slot.id]: "" };
  }
</script>

<div class="flex flex-col gap-2 p-2 overflow-y-auto">
  {#each slots as slot (slot.id)}
    {@const s = status[slot.id]}
    {@const sample = latest[slot.id]}
    {@const since = phaseSince[slot.id]}
    {@const recording = recordingInfo[slot.id]}
    <div
      class="border text-[11px] w-full"
      style="border-left: 4px solid {slotColor(colorIndexOf(slot.id))}"
    >
      <div
        class="flex items-center justify-between px-2 py-1 bg-gray-50 border-b"
      >
        <span class="font-medium">{slot.label}</span>
        <span
          class="text-[10px] px-1.5 rounded"
          class:text-green-700={s?.connected}
          class:bg-green-50={s?.connected}
          class:text-gray-400={!s?.connected}
        >
          {s?.connected ? "接続中" : "未接続"}
        </span>
      </div>
      <div class="grid grid-cols-2 gap-x-2 gap-y-0.5 px-2 py-1.5">
        <div class="text-gray-500">フェーズ</div>
        <div class="text-right font-mono">
          {rawValue(sample?.values, DataKind.Phase) ?? "-"}
        </div>
        <div class="text-gray-500">フェーズ経過</div>
        <div class="text-right font-mono">
          {since ? formatElapsed(nowMs - since.atMs) : "--:--"}
        </div>
        <div class="text-gray-500">衛星数</div>
        <div class="text-right font-mono">
          {numberValue(sample?.values, DataKind.NumSats) ?? "-"}
        </div>
        <div class="text-gray-500">バルブCAN生存</div>
        <div class="text-right font-mono">
          {numberValue(sample?.values, DataKind.FlagCanErrValve) ?? "-"}
        </div>
        <div class="text-gray-500">分離系CAN生存</div>
        <div class="text-right font-mono">
          {numberValue(sample?.values, DataKind.FlagCanErrSep) ?? "-"}
        </div>
      </div>
      {#if s}
        <div
          class="px-2 py-1 border-t text-[10px] text-gray-500 flex flex-wrap gap-x-2"
        >
          <span>正常 {s.goodCount}</span>
          <span>CRC異常 {s.crcErrors}</span>
          <span>長さ異常 {s.lengthErrors}</span>
          <span>不正行 {s.badLines}</span>
          <span>切断 {s.disconnects}</span>
        </div>
      {/if}
      {#if recording}
        <div
          class="px-2 py-1 border-t text-[10px] text-gray-500 truncate"
          title="{recording.rawPath}\n{recording.csvPath}"
        >
          💾 保存先: {recording.csvPath.split("/").pop()}
        </div>
      {/if}
      {#if slot.enabled}
        <div class="px-2 py-1.5 border-t flex gap-1">
          <Input
            value={drafts[slot.id] ?? ""}
            oninput={(e) =>
              (drafts = {
                ...drafts,
                [slot.id]: (e.target as HTMLInputElement).value,
              })}
            onkeydown={(e) => {
              if (e.key === "Enter") send(slot);
            }}
            autocapitalize="off"
            autocorrect="off"
            spellcheck={false}
            placeholder="送信するコマンドを入力"
            class="flex-1"
          />
          <button
            type="button"
            class="px-2 py-0.5 border text-[10px] hover:bg-gray-50 shrink-0"
            onclick={() => send(slot)}
          >
            送信
          </button>
        </div>
        <div class="px-2 pb-1.5 text-[10px] text-gray-400">
          入力内容をそのまま1byteずつ送信します（改行は付与されません）
        </div>
        {#if uplinkLog[slot.id]?.length}
          <div
            class="px-2 py-1 border-t text-[10px] text-gray-500 max-h-16 overflow-y-auto"
          >
            {#each [...uplinkLog[slot.id]].reverse() as entry (entry)}
              <div>{entry}</div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  {/each}
</div>
