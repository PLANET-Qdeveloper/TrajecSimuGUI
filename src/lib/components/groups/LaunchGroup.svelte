<script lang="ts">
  import type { LaunchConfig } from '$lib/types/config';
  import Input from '$lib/components/Input.svelte';
  import FilePathInput from '$lib/components/FilePathInput.svelte';

  let { launch = $bindable<LaunchConfig>() }: { launch: LaunchConfig } = $props();

  type WindMode = 'calm' | 'constant' | 'table';
  let windMode = $state<WindMode>(
    launch.wind_table ? 'table' : launch.wind_speed_mps !== undefined ? 'constant' : 'calm',
  );

  function onWindModeChange(mode: WindMode) {
    windMode = mode;
    if (mode === 'calm') {
      launch.wind_speed_mps = undefined;
      launch.wind_direction_deg = undefined;
      launch.wind_reference_alt = undefined;
      launch.wind_table = undefined;
    } else if (mode === 'constant') {
      launch.wind_table = undefined;
      if (launch.wind_speed_mps === undefined) launch.wind_speed_mps = 5.0;
      if (launch.wind_direction_deg === undefined) launch.wind_direction_deg = 270.0;
    } else {
      launch.wind_speed_mps = undefined;
      launch.wind_direction_deg = undefined;
      launch.wind_reference_alt = undefined;
    }
  }
</script>

<details open>
  <summary class="text-[11px] font-semibold text-gray-700 cursor-pointer select-none py-0.5">
    ▸ 打上げ条件
  </summary>
  <div class="mt-1 grid grid-cols-3 gap-x-2 gap-y-1">
    <!-- 座標・標高 -->
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">緯度 (°)</span>
      <Input type="number" step="0.00001" bind:value={launch.latitude} />
    </label>
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">経度 (°)</span>
      <Input type="number" step="0.00001" bind:value={launch.longitude} />
    </label>
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">標高 (m)</span>
      <Input type="number" step="0.1" bind:value={launch.elevation} />
    </label>

    <!-- レール・姿勢 -->
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">レール長 (m)</span>
      <Input type="number" step="0.1" min="0" bind:value={launch.rail_length} />
    </label>
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">ピッチ (°)</span>
      <Input type="number" step="0.1" bind:value={launch.pitch} />
    </label>
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">ヨー (°)</span>
      <Input type="number" step="0.1" bind:value={launch.yaw} />
    </label>

    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">ロール (°)</span>
      <Input type="number" step="0.1" bind:value={launch.roll} />
    </label>

    <!-- 風 -->
    <div class="col-span-3 flex flex-col gap-0.5 mt-0.5">
      <span class="text-[10px] text-gray-500">風設定</span>
      <div class="flex gap-2">
        {#each (['calm', 'constant', 'table'] as WindMode[]) as mode}
          <label class="flex items-center gap-1 text-xs cursor-pointer">
            <input
              type="radio"
              name="windMode"
              value={mode}
              checked={windMode === mode}
              onchange={() => onWindModeChange(mode)}
              class="accent-primary"
            />
            {mode === 'calm' ? '無風' : mode === 'constant' ? 'べき乗則' : 'テーブル'}
          </label>
        {/each}
      </div>
    </div>

    {#if windMode === 'constant'}
      <label class="flex flex-col gap-0.5">
        <span class="text-[10px] text-gray-500">風速 (m/s)</span>
        <Input type="number" step="0.1" min="0" bind:value={launch.wind_speed_mps} />
      </label>
      <label class="flex flex-col gap-0.5">
        <span class="text-[10px] text-gray-500">風向 (°)</span>
        <Input type="number" step="1" min="0" max="360" bind:value={launch.wind_direction_deg} />
      </label>
      <label class="flex flex-col gap-0.5">
        <span class="text-[10px] text-gray-500">基準高度 (m)</span>
        <Input type="number" step="0.1" bind:value={launch.wind_reference_alt} />
      </label>
      <label class="flex flex-col gap-0.5">
        <span class="text-[10px] text-gray-500">べき指数</span>
        <Input type="number" step="0.01" bind:value={launch.wind_power_exponent} />
      </label>
    {:else if windMode === 'table'}
      <div class="col-span-3">
        <FilePathInput label="風テーブル CSV" bind:value={launch.wind_table} extensions={['csv']} />
      </div>
    {/if}
  </div>
</details>
