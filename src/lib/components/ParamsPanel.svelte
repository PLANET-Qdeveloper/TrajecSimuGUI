<script lang="ts">
  import type { AppConfig } from "$lib/types/config";
  import Button from "$lib/components/Button.svelte";
  import LaunchGroup from "$lib/components/groups/LaunchGroup.svelte";
  import BodyGroup from "$lib/components/groups/BodyGroup.svelte";
  import EngineGroup from "$lib/components/groups/EngineGroup.svelte";
  import AeroGroup from "$lib/components/groups/AeroGroup.svelte";
  import ParachuteGroup from "$lib/components/groups/ParachuteGroup.svelte";
  import SimGroup from "$lib/components/groups/SimGroup.svelte";
  import GoogleSheetInput from "$lib/components/GoogleSheetInput.svelte";

  interface Props {
    config: AppConfig;
    configFilePath?: string;
    class?: string;
    url?: string;
    onsave?: () => void;
    onload?: () => void;
  }

  let {
    config = $bindable<AppConfig>(),
    configFilePath = "",
    class: cls = "",
    url = $bindable(""),
    onsave,
    onload,
  }: Props = $props();

  const filename = $derived(
    configFilePath
      ? (configFilePath.split("/").pop() ?? configFilePath)
      : "（未保存）",
  );

  function handleSheetMerge(merged: AppConfig) {
    config = merged;
  }
</script>

<div class="flex flex-col h-full overflow-hidden {cls}">
  <!-- ツールバー -->
  <div class="flex items-center gap-1 px-2 py-1 border-b bg-gray-50 shrink-0">
    <span
      class="text-[10px] text-gray-400 truncate max-w-[120px]"
      title={configFilePath || ""}
    >
      {filename}
    </span>
    <Button onclick={onsave}>保存</Button>
    <Button onclick={onload}>読込</Button>
  </div>

  <!-- Google スプレッドシート取込 -->
  <div class="px-2 py-1 border-b bg-gray-50 shrink-0">
    <GoogleSheetInput {config} onmerge={handleSheetMerge} bind:url />
  </div>

  <!-- パラメータ グループ (スクロール) -->
  <div class="flex-1 overflow-y-auto px-2 py-1 space-y-2">
    <LaunchGroup bind:launch={config.launch} />
    <BodyGroup bind:body={config.body} />
    <EngineGroup bind:engine={config.engine} />
    <AeroGroup bind:aero={config.aero} />
    <ParachuteGroup bind:parachute={config.parachute} />
    <SimGroup bind:sim={config.sim} />
  </div>
</div>
