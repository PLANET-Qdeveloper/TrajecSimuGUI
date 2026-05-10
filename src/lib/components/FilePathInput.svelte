<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';

  interface Props {
    value?: string;
    label: string;
    extensions?: string[];
    filterName?: string;
    placeholder?: string;
    defaultDir?: string;
  }

  let {
    value = $bindable(''),
    label,
    extensions = ['csv'],
    filterName,
    placeholder = '未設定',
    defaultDir = '',
  }: Props = $props();

  const filterLabel = $derived(filterName ?? extensions[0]?.toUpperCase() ?? 'FILE');

  async function browse() {
    const result = await open({
      multiple: false,
      defaultPath: defaultDir || undefined,
      filters: [{ name: filterLabel, extensions }],
    });
    if (result) value = result as string;
  }
</script>

<div class="flex flex-col gap-0.5">
  <span class="text-[10px] text-gray-500 font-medium uppercase tracking-wide">{label}</span>
  <div class="flex gap-1">
    <input
      class="flex-1 px-2 py-0.5 border text-xs bg-white focus:outline-none focus:ring-1 focus:ring-primary truncate"
      value={value || ''}
      readonly
      {placeholder}
      title={value || placeholder}
    />
    <button
      onclick={browse}
      class="shrink-0 px-2 py-0.5 text-xs border bg-white hover:bg-gray-50 active:bg-gray-100"
    >参照</button>
    {#if value}
      <button
        onclick={() => (value = '')}
        class="shrink-0 px-1.5 py-0.5 text-xs border bg-white hover:bg-gray-50 text-gray-400"
        title="クリア"
      >×</button>
    {/if}
  </div>
</div>
