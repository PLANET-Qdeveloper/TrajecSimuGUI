<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import * as yaml from 'js-yaml';

  import { defaultConfig, type AppConfig, type SimSummary } from '$lib/types/config';
  import { parseConfig, serializeConfig } from '$lib/utils/yamlConfig';
  import { dirOf } from '$lib/utils/path';

  import Map from '$lib/components/Map.svelte';
  import ParamsPanel from '$lib/components/ParamsPanel.svelte';
  import RunPanel from '$lib/components/RunPanel.svelte';

  let activeTab = $state<'params' | 'map'>('params');
  let config = $state<AppConfig>(defaultConfig());
  let configFilePath = $state('');
  let configBaseDir = $state('');

  let running = $state(false);
  let progressMsg = $state('');
  let result = $state<SimSummary | null>(null);

  onMount(() => {
    const unlisten = listen<string>('sim-progress', (e) => {
      progressMsg = e.payload;
    });
    return () => { unlisten.then((fn) => fn()); };
  });

  async function loadFromPath(path: string) {
    try {
      const content = await invoke<string>('read_text_file', { path });
      const parsed = yaml.load(content);
      const baseDir = dirOf(path);
      config = parseConfig(parsed, baseDir);
      configFilePath = path;
      configBaseDir = baseDir;
    } catch (e) {
      alert(`読み込みエラー: ${e}`);
    }
  }

  async function handleLoad() {
    const path = await open({
      multiple: false,
      filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }],
    });
    if (path) await loadFromPath(path as string);
  }

  async function handleSave() {
    if (!configFilePath) return handleSaveAs();
    try {
      const content = serializeConfig(config, configBaseDir);
      await invoke('write_text_file', { path: configFilePath, content });
    } catch (e) {
      alert(`保存エラー: ${e}`);
    }
  }

  async function handleSaveAs() {
    const path = await save({
      defaultPath: configFilePath || undefined,
      filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }],
    });
    if (!path) return;
    const dir = dirOf(path as string);
    try {
      const content = serializeConfig(config, dir);
      await invoke('write_text_file', { path, content });
      configFilePath = path as string;
      configBaseDir = dir;
    } catch (e) {
      alert(`保存エラー: ${e}`);
    }
  }

  async function handleImport() {
    const path = await open({
      multiple: false,
      filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }],
    });
    if (!path) return;
    try {
      const content = await invoke<string>('read_text_file', { path: path as string });
      const parsed = yaml.load(content);
      const baseDir = dirOf(path as string);
      config = parseConfig(parsed, baseDir);
      // configFilePath はそのまま（インポートは現在ファイルを変えない）
    } catch (e) {
      alert(`インポートエラー: ${e}`);
    }
  }

  async function handleRun(outDir: string, noDem: boolean) {
    running = true;
    result = null;
    progressMsg = '';
    try {
      result = await invoke<SimSummary>('run_simulation', {
        config,
        outDir,
        noDem,
      });
    } catch (e) {
      progressMsg = `エラー: ${e}`;
    } finally {
      running = false;
    }
  }
</script>

<div class="flex flex-col h-screen overflow-hidden bg-white text-black">
  <!-- タブバー -->
  <div class="flex border-b shrink-0">
    {#each (['params', 'map'] as const) as tab}
      <button
        onclick={() => (activeTab = tab)}
        class="px-4 py-1.5 text-xs font-medium border-b-2 transition-colors
               {activeTab === tab
               ? 'border-primary text-primary'
               : 'border-transparent text-gray-500 hover:text-gray-700'}"
      >
        {tab === 'params' ? 'パラメータ' : 'マップ'}
      </button>
    {/each}
  </div>

  <!-- コンテンツ -->
  <div class="flex-1 overflow-hidden">
    {#if activeTab === 'params'}
      <div class="flex h-full overflow-hidden">
        <!-- 左カラム: パラメータ + 実行パネル -->
        <div class="flex flex-col w-[480px] min-w-[480px] border-r overflow-hidden">
          <ParamsPanel
            bind:config
            configFilePath={configFilePath}
            class="flex-1 overflow-hidden"
            onsave={handleSave}
            onload={handleLoad}
            onimport={handleImport}
          />
          <RunPanel
            {config}
            bind:running
            bind:progressMsg
            bind:result
            class="border-t shrink-0"
            onrun={handleRun}
          />
        </div>

        <!-- 右カラム: 将来の分析パラメータ用予約スペース -->
        <div class="flex-1 flex flex-col p-2 bg-gray-50 overflow-hidden">
          <p class="text-[10px] text-gray-400 font-medium uppercase tracking-wide">
            分析パラメータ（予定）
          </p>
        </div>
      </div>
    {:else}
      <Map />
    {/if}
  </div>
</div>
