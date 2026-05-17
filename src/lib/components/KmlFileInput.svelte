<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';

  let {
    filename = '',
    onload,
    onclear,
  }: {
    filename?: string;
    onload: (kmlStr: string, filename: string) => void;
    onclear: () => void;
  } = $props();

  async function handleOpen() {
    const path = await open({
      multiple: false,
      filters: [{ name: 'KML / KMZ', extensions: ['kml', 'kmz'] }],
    });
    if (!path) return;
    try {
      const kmlStr = await invoke<string>('load_kml_file', { path: path as string });
      const name = (path as string).split('/').pop() ?? '';
      onload(kmlStr, name);
    } catch (e) {
      alert(`KML 読み込みエラー: ${e}`);
    }
  }
</script>

<div class="flex items-center gap-2 px-2.5 py-1.5 border-t border-gray-200 bg-white text-[11px] shrink-0">
  <button onclick={handleOpen} class="px-2.5 py-1 bg-sky-500 text-white rounded hover:bg-sky-600 cursor-pointer whitespace-nowrap">
    KML / KMZ を開く
  </button>
  {#if filename}
    <span class="text-gray-700 truncate max-w-[200px]">{filename}</span>
    <button onclick={onclear} class="text-gray-400 hover:text-gray-800 cursor-pointer text-xs leading-none">✕</button>
  {/if}
</div>