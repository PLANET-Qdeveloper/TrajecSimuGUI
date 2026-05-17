<script lang="ts">
  import type { ParachuteConfig } from "$lib/types/config";
  import Input from "$lib/components/Input.svelte";
  import FilePathInput from "$lib/components/FilePathInput.svelte";

  interface Props {
    parachute?: ParachuteConfig;
  }
  let { parachute = $bindable<ParachuteConfig | undefined>() }: Props =
    $props();

  let enabled = $state(true);
  let hasInitialized = false;

  $effect(() => {
    if (!hasInitialized) {
      hasInitialized = true;
      if (parachute === undefined) {
        parachute = { terminal_velocity_table: "", deploy_delay_sec: 1.0 };
      }
    }

    enabled = parachute !== undefined;
  });

  function toggle() {
    enabled = !enabled;
    if (enabled && !parachute) {
      parachute = { terminal_velocity_table: "", deploy_delay_sec: 1.0 };
    } else if (!enabled) {
      parachute = undefined;
    }
  }
</script>

<details open>
  <summary
    class="text-[11px] font-semibold text-gray-700 cursor-pointer select-none py-0.5"
  >
    ▸ パラシュート
  </summary>
  <div class="mt-1 flex flex-col gap-1.5">
    <label class="flex items-center gap-1.5 text-xs cursor-pointer">
      <input
        type="checkbox"
        checked={enabled}
        onchange={toggle}
        class="accent-primary"
      />
      パラシュートを使用する
    </label>

    {#if enabled && parachute}
      <FilePathInput
        label="終端速度テーブル CSV"
        bind:value={parachute.terminal_velocity_table}
        extensions={["csv"]}
      />
      <label class="flex flex-col gap-0.5 w-36">
        <span class="text-[10px] text-gray-500">展開遅延 (s)</span>
        <Input
          type="number"
          step="0.1"
          min="0"
          bind:value={parachute.deploy_delay_sec}
        />
      </label>
    {/if}
  </div>
</details>
