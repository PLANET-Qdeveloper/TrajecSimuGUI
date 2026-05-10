<script lang="ts">
  import type { EngineConfig } from '$lib/types/config';
  import Input from '$lib/components/Input.svelte';
  import FilePathInput from '$lib/components/FilePathInput.svelte';

  let { engine = $bindable<EngineConfig>() }: { engine: EngineConfig } = $props();

  function pos3(
    arr: [number, number, number],
    i: number,
    val: string,
  ): [number, number, number] {
    const v = [...arr] as [number, number, number];
    v[i] = parseFloat(val) || 0;
    return v;
  }
</script>

<details open>
  <summary class="text-[11px] font-semibold text-gray-700 cursor-pointer select-none py-0.5">
    ▸ エンジン
  </summary>
  <div class="mt-1 flex flex-col gap-1.5">
    <!-- 推力テーブル -->
    <FilePathInput label="推力テーブル CSV" bind:value={engine.thrust_table} extensions={['csv']} />

    <!-- スラスタ位置 -->
    <div class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">スラスタ位置 (m) — X, Y, Z</span>
      <div class="grid grid-cols-3 gap-1">
        {#each [0, 1, 2] as i}
          <Input
            type="number" step="0.001"
            value={engine.thruster_pos[i]}
            oninput={(e) => (engine.thruster_pos = pos3(engine.thruster_pos, i, (e.target as HTMLInputElement).value))}
          />
        {/each}
      </div>
    </div>

    <!-- タンク -->
    <div class="border-t pt-1 flex flex-col gap-1">
      <span class="text-[10px] font-medium text-gray-600">タンク</span>
      <div class="grid grid-cols-3 gap-x-2 gap-y-1">
        <label class="flex flex-col gap-0.5">
          <span class="text-[10px] text-gray-500">タンク質量 (kg)</span>
          <Input type="number" step="0.01" min="0" bind:value={engine.tank.tank_contents} />
        </label>
        <div class="col-span-2"></div>
        <div class="col-span-3 flex flex-col gap-0.5">
          <span class="text-[10px] text-gray-500">タンク位置 (m) — X, Y, Z</span>
          <div class="grid grid-cols-3 gap-1">
            {#each [0, 1, 2] as i}
              <Input
                type="number" step="0.001"
                value={engine.tank.position[i]}
                oninput={(e) => (engine.tank.position = pos3(engine.tank.position, i, (e.target as HTMLInputElement).value))}
              />
            {/each}
          </div>
        </div>
      </div>
    </div>

    <!-- 燃料 -->
    <div class="border-t pt-1 flex flex-col gap-1">
      <span class="text-[10px] font-medium text-gray-600">燃料</span>
      <div class="grid grid-cols-3 gap-x-2 gap-y-1">
        <label class="flex flex-col gap-0.5">
          <span class="text-[10px] text-gray-500">燃料質量 (kg)</span>
          <Input type="number" step="0.001" min="0" bind:value={engine.fuel.fuel_section_weight} />
        </label>
        <label class="flex flex-col gap-0.5">
          <span class="text-[10px] text-gray-500">燃焼後質量 (kg)</span>
          <Input type="number" step="0.001" min="0" bind:value={engine.fuel.fuel_section_weight_after_burn} />
        </label>
        <div></div>
        <div class="col-span-3 flex flex-col gap-0.5">
          <span class="text-[10px] text-gray-500">燃料位置 (m) — X, Y, Z</span>
          <div class="grid grid-cols-3 gap-1">
            {#each [0, 1, 2] as i}
              <Input
                type="number" step="0.001"
                value={engine.fuel.position[i]}
                oninput={(e) => (engine.fuel.position = pos3(engine.fuel.position, i, (e.target as HTMLInputElement).value))}
              />
            {/each}
          </div>
        </div>
      </div>
    </div>
  </div>
</details>
