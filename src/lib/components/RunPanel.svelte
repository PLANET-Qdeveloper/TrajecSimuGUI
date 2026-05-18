<script lang="ts">
    import {open} from "@tauri-apps/plugin-dialog";
    import type {SimSummary} from "$lib/types/config";
    import Button from "$lib/components/Button.svelte";

    interface Props {
        running?: boolean;
        progressMsg?: string;
        result?: SimSummary | null;
        outDir?: string;
        noDem?: boolean;
        class?: string;
        on_run_single?: () => void;
        on_run_parallel?: () => void;
    }

    let {
        running = $bindable(false),
        progressMsg = $bindable(""),
        result = $bindable<SimSummary | null>(null),
        outDir = $bindable(""),
        noDem = $bindable(false),
        class: cls = "",
        on_run_single,
        on_run_parallel,
    }: Props = $props();

    async function browseOutDir() {
        const dir = await open({directory: true, multiple: false});
        if (dir) outDir = dir as string;
    }

    function handleRunSingle() {
        if (!outDir) {
            alert("出力ディレクトリを選択してください");
            return;
        }
        on_run_single?.();
    }

    function handleRunParallel() {
        if (!outDir) {
            alert("出力ディレクトリを選択してください");
            return;
        }
        on_run_parallel?.();
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
            <Button
                    onclick={browseOutDir}
            >参照
            </Button>
        </div>
    </div>

    <!-- オプション -->
    <div class="flex items-center gap-3">
        <label class="flex items-center gap-1 text-xs cursor-pointer">
            <input type="checkbox" bind:checked={noDem} class="accent-primary"/>
            DEM 補正なし
        </label>
    </div>

    <!-- 実行ボタン -->
    <div class="w-full flex gap-1.5">
        <Button
                onclick={handleRunSingle}
                disabled={running}
        >
            {running ? "実行中..." : "▶ シングルシミュレーション実行"}
        </Button>
        <Button
                onclick={handleRunParallel}
                disabled={running}
                class="w-full py-1 text-sm font-medium text-white bg-primary hover:bg-primary-light
           disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
            {running ? "実行中..." : "▶ 着地範囲シミュレーション実行"}
        </Button>
    </div>

    <!-- 進捗 -->
    {#if running || progressMsg}
        <p class="text-[11px] text-gray-500">{progressMsg}</p>
    {/if}

    <!-- 結果 -->
    {#if result}
        <div class="border-t pt-1.5 flex flex-col gap-0.5">
      <span class="text-[10px] font-semibold text-gray-600"
      >シミュレーション結果</span
      >
            <div class="grid grid-cols-2 gap-x-4 gap-y-0.5 text-xs">
                <span class="text-gray-500">アポジー</span>
                <span class="font-mono">{result.apogee_m.toFixed(0)} m</span>
                <span class="text-gray-500">最大速度</span>
                <span class="font-mono">{result.max_speed_mps.toFixed(1)} m/s</span>
                <span class="text-gray-500">飛行時間</span>
                <span class="font-mono">{result.flight_time_sec.toFixed(1)} s</span>
            </div>
            <p class="text-[10px] text-gray-400 mt-0.5">出力: {result.out_dir}</p>
        </div>
    {/if}
</div>
