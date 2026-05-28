<script lang="ts">
  import type { AppConfig } from "$lib/types/config";
  import type { CsvTableDataMap, TableKey } from "$lib/utils/csvTable";
  import { defaultCsvTableDataMap } from "$lib/utils/csvTable";
  import TableEditor from "$lib/components/TableEditor.svelte";

  interface Props {
    tableData: CsvTableDataMap;
    config: AppConfig;
  }

  let {
    tableData = $bindable(defaultCsvTableDataMap()),
    config,
  }: Props = $props();

  type TableEntry = { key: TableKey; label: string; filePath: () => string };

  const TABLE_ENTRIES: TableEntry[] = [
    {
      key: "thrust_table",
      label: "推力テーブル",
      filePath: () => config.engine.thrust_table,
    },
    {
      key: "cp_mach_table",
      label: "CP–マッハ",
      filePath: () => config.aero.cp_mach_table,
    },
    {
      key: "cd0_alpha_mach_table",
      label: "Cd0–α–マッハ",
      filePath: () => config.aero.cd0_alpha_mach_table,
    },
    {
      key: "cn_table",
      label: "Cn–マッハ",
      filePath: () => config.aero.cn_table,
    },
    {
      key: "cs_table",
      label: "Cs–マッハ",
      filePath: () => config.aero.cs_table,
    },
    {
      key: "wind_table",
      label: "風テーブル",
      filePath: () => config.launch.wind_table ?? "",
    },
    {
      key: "terminal_velocity_table",
      label: "終端速度",
      filePath: () => config.parachute?.terminal_velocity_table ?? "",
    },
  ];

  let selectedKey = $state<TableKey>("thrust_table");
  const selectedEntry = $derived(TABLE_ENTRIES.find((e) => e.key === selectedKey)!);
</script>

<div class="flex h-full overflow-hidden text-xs">
  <!-- 左: テーブル一覧 -->
  <div class="w-36 shrink-0 border-r overflow-y-auto flex flex-col bg-white">
    {#each TABLE_ENTRIES as entry (entry.key)}
      {@const hasData = tableData[entry.key].headers.length > 0}
      {@const rowCount = tableData[entry.key].rows.length}
      <button
        type="button"
        onclick={() => (selectedKey = entry.key)}
        class="flex flex-col items-start px-2 py-1.5 border-b border-gray-100 text-left transition-colors
               {selectedKey === entry.key
                 ? 'bg-primary/10 text-primary font-medium'
                 : 'hover:bg-gray-50 text-gray-700'}"
      >
        <span class="text-[11px] leading-tight">{entry.label}</span>
        {#if !hasData}
          <span class="text-[10px] text-amber-500">未読込</span>
        {:else}
          <span class="text-[10px] text-gray-400">{rowCount}行</span>
        {/if}
      </button>
    {/each}
  </div>

  <!-- 右: 選択テーブルのエディタ -->
  <div class="flex-1 overflow-hidden flex flex-col">
    <!-- ヘッダ -->
    <div class="px-3 py-2 border-b bg-gray-50 shrink-0">
      <div class="text-[11px] font-semibold text-gray-700">
        {selectedEntry.label}
      </div>
      {#if selectedEntry.filePath()}
        <div
          class="text-[10px] text-gray-400 truncate mt-0.5"
          title={selectedEntry.filePath()}
        >
          {selectedEntry.filePath()}
        </div>
      {:else}
        <div class="text-[10px] text-gray-400 italic mt-0.5">
          ファイルパス未設定（パラメータタブで設定）
        </div>
      {/if}
    </div>

    <!-- テーブルエディタ -->
    <div class="flex-1 overflow-auto p-3">
      {#key selectedKey}
        <TableEditor bind:data={tableData[selectedKey]} />
      {/key}
    </div>
  </div>
</div>
