<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import type { AppConfig, SimSummary } from '$lib/types/config';

  interface Props {
    config: AppConfig;
    running?: boolean;
    progressMsg?: string;
    result?: SimSummary | null;
    class?: string;
    onrun?: (outDir: string, noDem: boolean) => void;
  }

  let {
    config,
    running = $bindable(false),
    progressMsg = $bindable(''),
    result = $bindable<SimSummary | null>(null),
    class: cls = '',
    onrun,
  }: Props = $props();

  let outDir = $state('');
  let noDem = $state(false);

  async function browseOutDir() {
    const dir = await open({ directory: true, multiple: false });
    if (dir) outDir = dir as string;
  }

  function handleRun() {
    if (!outDir) {
      alert('出力ディレクトリを選択してください');
      return;
    }
    onrun?.(outDir, noDem);
  }
</script>

<div class="flex flex-col gap-1.5 p-2 bg-gray-50 {cls}">
  <!-- 出力ディレクトリ -->
  <div class="flex flex-col gap-0.5">
    <span class="text-[10px] text-gray-500">出力ディレクトリ</span>
    <div class="flex gap-1">
      <input
        class="flex-1 px-2 py-0.5 border text-xs bg-white"
        value={outDir}
        readonly
        placeholder="未選択"
        title={outDir}
      />
      <button
        onclick={browseOutDir}
        class="shrink-0 px-2 py-0.5 text-xs border bg-white hover:bg-gray-50"
      >参照</button>
    </div>
  </div>

  <!-- オプション -->
  <div class="flex items-center gap-3">
    <label class="flex items-center gap-1 text-xs cursor-pointer">
      <input type="checkbox" bind:checked={noDem} class="accent-primary" />
      DEM 補正なし
    </label>
  </div>

  <!-- 実行ボタン -->
  <button
    onclick={handleRun}
    disabled={running}
    class="w-full py-1 text-sm font-medium text-white bg-primary hover:bg-primary-light
           disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
  >
    {running ? '実行中...' : '▶ シミュレーション実行'}
  </button>

  <!-- 進捗 -->
  {#if running || progressMsg}
    <p class="text-[11px] text-gray-500">{progressMsg}</p>
  {/if}

  <!-- 結果 -->
  {#if result}
    <div class="border-t pt-1.5 flex flex-col gap-0.5">
      <span class="text-[10px] font-semibold text-gray-600">シミュレーション結果</span>
      <div class="grid grid-cols-2 gap-x-4 gap-y-0.5 text-xs">
        <span class="text-gray-500">アポジー</span>
        <span class="font-mono">{result.apogee_m.toFixed(0)} m</span>
        <span class="text-gray-500">最大速度</span>
        <span class="font-mono">{result.max_speed_mps.toFixed(1)} m/s</span>
        <span class="text-gray-500">飛行時間</span>
        <span class="font-mono">{result.flight_time_sec.toFixed(1)} s</span>
        {#if result.landing_lat !== undefined}
          <span class="text-gray-500">着地（{result.landing_source}）</span>
          <span class="font-mono text-[10px]">
            {result.landing_lat.toFixed(5)}°, {result.landing_lon?.toFixed(5)}°
          </span>
        {/if}
      </div>
      <p class="text-[10px] text-gray-400 mt-0.5">出力: {result.out_dir}</p>
    </div>
  {/if}
</div>
