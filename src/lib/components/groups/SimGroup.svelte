<script lang="ts">
  import type { SimConfig } from "$lib/types/config";
  import Input from "$lib/components/Input.svelte";

  let { sim = $bindable<SimConfig>() }: { sim: SimConfig } = $props();
</script>

<details open>
  <summary
    class="text-[11px] font-semibold text-gray-700 cursor-pointer select-none py-0.5"
  >
    ▸ シミュレーション設定
  </summary>
  <div class="mt-1 grid grid-cols-2 gap-x-2 gap-y-1">
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">最大飛行時間 (s)</span>
      <Input type="number" step="1" min="1" bind:value={sim.flight_duration} />
    </label>
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">時間刻み (s)</span>
      <Input
        type="number"
        step="0.001"
        min="0.0001"
        bind:value={sim.time_step}
      />
    </label>
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">CSV 間引き</span>
      <Input
        type="number"
        step="1"
        min="1"
        bind:value={sim.csv_sample_interval}
      />
    </label>
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">KML 間引き</span>
      <Input
        type="number"
        step="1"
        min="1"
        bind:value={sim.kml_sample_interval}
      />
    </label>
  </div>
</details>
