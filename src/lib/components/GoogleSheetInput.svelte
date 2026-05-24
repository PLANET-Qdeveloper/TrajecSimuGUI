<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { mergeSheetConfig, countUpdatedFields } from "$lib/utils/configMerge";
  import type { SheetConfig } from "$lib/utils/configMerge";
  import type { AppConfig } from "$lib/types/config";
  import Button from "$lib/components/Button.svelte";

  interface Props {
    config: AppConfig;
    onmerge?: (config: AppConfig) => void;
    url: string;
  }

  let { config, onmerge, url = $bindable("") }: Props = $props();

  let loading = $state(false);
  let loggedIn = $state(false);
  let errorMsg = $state<string | null>(null);
  let successCount = $state<number | null>(null);

  onMount(async () => {
    loggedIn = await invoke<boolean>("get_google_auth_status");
  });

  async function handleLoad() {
    if (!url.trim()) return;
    errorMsg = null;
    successCount = null;
    loading = true;
    try {
      const sheet = await invoke<SheetConfig>("fetch_google_sheet", { url });
      loggedIn = true;
      const merged = mergeSheetConfig(config, sheet);
      successCount = countUpdatedFields(sheet);
      onmerge?.(merged);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      loading = false;
    }
  }

  async function handleRevoke() {
    try {
      await invoke("revoke_google_auth");
      loggedIn = false;
      successCount = null;
      errorMsg = null;
    } catch (e) {
      errorMsg = String(e);
    }
  }
</script>

<div class="flex flex-col gap-1">
  <!-- URL 入力行 -->
  <div class="flex items-center gap-1">
    <input
      type="text"
      bind:value={url}
      placeholder="スプレッドシート URL"
      class="flex-1 px-2 py-0.5 text-xs border bg-white focus:outline-none focus:ring-1 focus:ring-primary focus:border-transparent min-w-0"
      onkeydown={(e) => e.key === "Enter" && handleLoad()}
    />
    <Button onclick={handleLoad} disabled={loading || !url.trim()}>
      {loading ? "取込中..." : "諸元取込"}
    </Button>
  </div>

  <!-- ステータス行 -->
  <div class="flex items-center gap-2 text-[10px]">
    {#if loggedIn}
      <span class="text-green-600">● ログイン済み</span>
      <Button onclick={handleRevoke}>ログアウト</Button>
    {:else}
      <span class="text-gray-400"
        >○ 未ログイン（読込時にブラウザが開きます）</span
      >
    {/if}

    {#if successCount !== null}
      <span class="text-blue-600 ml-auto"
        >{successCount} 項目を取込みました</span
      >
    {/if}
    {#if errorMsg}
      <span class="text-red-500 ml-auto" title={errorMsg}>
        エラー: {errorMsg.slice(0, 60)}{errorMsg.length > 60 ? "…" : ""}
      </span>
    {/if}
  </div>
</div>
