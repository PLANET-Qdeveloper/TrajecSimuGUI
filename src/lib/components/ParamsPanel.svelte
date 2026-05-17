<script lang="ts">
  import type { AppConfig } from "$lib/types/config";
  import LaunchGroup from "$lib/components/groups/LaunchGroup.svelte";
  import BodyGroup from "$lib/components/groups/BodyGroup.svelte";
  import EngineGroup from "$lib/components/groups/EngineGroup.svelte";
  import AeroGroup from "$lib/components/groups/AeroGroup.svelte";
  import ParachuteGroup from "$lib/components/groups/ParachuteGroup.svelte";
  import SimGroup from "$lib/components/groups/SimGroup.svelte";

  interface Props {
    config: AppConfig;
    configFilePath?: string;
    class?: string;
    onsave?: () => void;
    onload?: () => void;
    onimport?: () => void;
  }

  let {
    config = $bindable<AppConfig>(),
    configFilePath = "",
    class: cls = "",
    onsave,
    onload,
    onimport,
  }: Props = $props();

  const filename = $derived(
    configFilePath
      ? (configFilePath.split("/").pop() ?? configFilePath)
      : "（未保存）",
  );
</script>

<div class="flex flex-col h-full overflow-hidden {cls}">
  <!-- ツールバー -->
  <div class="flex items-center gap-1 px-2 py-1 border-b bg-gray-50 shrink-0">
    <span
      class="text-[10px] text-gray-400 truncate flex-1"
      title={configFilePath || ""}
    >
      {filename}
    </span>
    <button
      onclick={onsave}
      class="px-2 py-0.5 text-xs border bg-white hover:bg-gray-50 active:bg-gray-100 shrink-0"
      >保存</button
    >
    <button
      onclick={onload}
      class="px-2 py-0.5 text-xs border bg-white hover:bg-gray-50 active:bg-gray-100 shrink-0"
      >読込</button
    >
    <button
      onclick={onimport}
      class="px-2 py-0.5 text-xs border bg-white hover:bg-gray-50 active:bg-gray-100 shrink-0"
      >インポート</button
    >
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
