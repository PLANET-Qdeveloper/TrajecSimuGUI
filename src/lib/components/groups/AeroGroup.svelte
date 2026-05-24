<script lang="ts">
  import type { AeroConfig } from "$lib/types/config";
  import NumberInput from "$lib/components/NumberInput.svelte";
  import FilePathInput from "$lib/components/FilePathInput.svelte";

  let { aero = $bindable<AeroConfig>() }: { aero: AeroConfig } = $props();

  function pos3(
    arr: [number, number, number],
    i: number,
    v: number,
  ): [number, number, number] {
    const next = [...arr] as [number, number, number];
    next[i] = v;
    return next;
  }
</script>

<details open>
  <summary
    class="text-[11px] font-semibold text-gray-700 cursor-pointer select-none py-0.5"
  >
    ▸ 空力
  </summary>
  <div class="mt-1 flex flex-col gap-1.5">
    <!-- CP at launch -->
    <div class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">初期 CP 位置 (m) — X, Y, Z</span>
      <div class="grid grid-cols-3 gap-1">
        {#each [0, 1, 2] as i (i)}
          <NumberInput
            step={0.001}
            value={aero.cp_at_launch[i]}
            onValueChange={(v) =>
              (aero.cp_at_launch = pos3(aero.cp_at_launch, i, v))}
          />
        {/each}
      </div>
    </div>

    <!-- Table files -->
    <FilePathInput
      label="CP–マッハ テーブル CSV"
      bind:value={aero.cp_mach_table}
      extensions={["csv"]}
    />
    <FilePathInput
      label="Cd0–α–マッハ テーブル CSV"
      bind:value={aero.cd0_alpha_mach_table}
      extensions={["csv"]}
    />
    <FilePathInput
      label="Cn–マッハ テーブル CSV"
      bind:value={aero.cn_table}
      extensions={["csv"]}
    />
    <FilePathInput
      label="Cs–マッハ テーブル CSV"
      bind:value={aero.cs_table}
      extensions={["csv"]}
    />

    <!-- Damping -->
    <div class="grid grid-cols-3 gap-x-2 gap-y-1">
      <label class="flex flex-col gap-0.5">
        <span class="text-[10px] text-gray-500">ロール減衰</span>
        <NumberInput step={0.001} bind:value={aero.roll_damping} />
      </label>
      <label class="flex flex-col gap-0.5">
        <span class="text-[10px] text-gray-500">ピッチ減衰</span>
        <NumberInput step={0.001} bind:value={aero.pitch_damping} />
      </label>
      <label class="flex flex-col gap-0.5">
        <span class="text-[10px] text-gray-500">ヨー減衰</span>
        <NumberInput step={0.001} bind:value={aero.yaw_damping} />
      </label>
    </div>
  </div>
</details>
