<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import Button from "$lib/components/Button.svelte";

  interface Props {
    value?: string;
    label: string;
    extensions?: string[];
    filterName?: string;
    placeholder?: string;
    defaultDir?: string;
    directory?: boolean;
  }

  let {
    value = $bindable(),
    label,
    extensions = ["csv"],
    filterName,
    placeholder = "未設定",
    defaultDir = "",
    directory = false,
  }: Props = $props();

  const filterLabel = $derived(
    filterName ?? extensions[0]?.toUpperCase() ?? "FILE",
  );

  async function browse() {
    if (directory) {
      const result = await open({ directory: true, multiple: false });
      if (result) value = result as string;
    } else {
      const result = await open({
        multiple: false,
        defaultPath: defaultDir || undefined,
        filters: [{ name: filterLabel, extensions }],
      });
      if (result) value = result as string;
    }
  }
</script>

<div class="flex flex-col gap-0.5">
  <span class="text-[10px] text-gray-500 font-medium uppercase tracking-wide"
    >{label}</span
  >
  <div class="flex gap-1">
    <input
      class="flex-1 px-2 py-0.5 border text-xs bg-white focus:outline-none focus:ring-1 focus:ring-primary truncate"
      value={value || ""}
      readonly
      {placeholder}
      title={value || placeholder}
    />
    <Button onclick={browse}>参照</Button>
    {#if value}
      <Button onclick={() => (value = undefined)}>×</Button>
    {/if}
  </div>
</div>
