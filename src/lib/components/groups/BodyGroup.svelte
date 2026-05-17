<script lang="ts">
  import type { BodyConfig } from "$lib/types/config";
  import Input from "$lib/components/Input.svelte";

  let { body = $bindable<BodyConfig>() }: { body: BodyConfig } = $props();
</script>

<details open>
  <summary
    class="text-[11px] font-semibold text-gray-700 cursor-pointer select-none py-0.5"
  >
    ▸ 機体
  </summary>
  <div class="mt-1 grid grid-cols-3 gap-x-2 gap-y-1">
    <label class="flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">直径 (m)</span>
      <Input type="number" step="0.001" min="0" bind:value={body.diameter} />
    </label>
    <label class="col-span-2 flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">乾燥質量（燃料部含む） (kg)</span>
      <Input
        type="number"
        step="0.01"
        min="0"
        bind:value={body.dry_mass_with_fuel_section}
      />
    </label>

    <!-- 重心 -->
    <div class="col-span-3 flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500">重心 (m) — X, Y, Z</span>
      <div class="grid grid-cols-3 gap-1">
        {#each [0, 1, 2] as i (i)}
          <Input
            type="number"
            step="0.001"
            value={body.cg[i]}
            oninput={(e) => {
              const v = [...body.cg] as [number, number, number];
              v[i] = parseFloat((e.target as HTMLInputElement).value) || 0;
              body.cg = v;
            }}
          />
        {/each}
      </div>
    </div>

    <!-- 慣性テンソル -->
    <div class="col-span-3 flex flex-col gap-0.5">
      <span class="text-[10px] text-gray-500"
        >慣性テンソル (kg·m²) — Ixx, Iyy, Izz, Ixy, Ixz, Iyz</span
      >
      <div class="grid grid-cols-3 gap-1">
        {#each ["Ixx", "Iyy", "Izz", "Ixy", "Ixz", "Iyz"] as label, i (label)}
          <label class="flex flex-col gap-0.5">
            <span class="text-[10px] text-gray-400">{label}</span>
            <Input
              type="number"
              step="0.001"
              value={body.inertia[i]}
              oninput={(e) => {
                const v = [...body.inertia] as [
                  number,
                  number,
                  number,
                  number,
                  number,
                  number,
                ];
                v[i] = parseFloat((e.target as HTMLInputElement).value) || 0;
                body.inertia = v;
              }}
            />
          </label>
        {/each}
      </div>
    </div>
  </div>
</details>
