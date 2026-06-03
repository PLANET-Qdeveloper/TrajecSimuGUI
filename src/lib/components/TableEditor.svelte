<script lang="ts">
  import type { CsvTableData } from "$lib/utils/csvTable";

  interface Props {
    data: CsvTableData;
  }

  let { data = $bindable() }: Props = $props();

  function addRow() {
    data = {
      ...data,
      rows: [...data.rows, Array(data.headers.length).fill("")],
    };
  }

  function deleteRow(i: number) {
    data = { ...data, rows: data.rows.filter((_, idx) => idx !== i) };
  }

  function addColumn() {
    const name = `col${data.headers.length + 1}`;
    data = {
      headers: [...data.headers, name],
      rows: data.rows.map((row) => [...row, ""]),
    };
  }

  function deleteColumn(j: number) {
    data = {
      headers: data.headers.filter((_, idx) => idx !== j),
      rows: data.rows.map((row) => row.filter((_, idx) => idx !== j)),
    };
  }
</script>

{#if data.headers.length === 0}
  <div class="text-[11px] text-gray-400 italic py-4 text-center">
    データ未読込 — パラメータタブでCSVファイルパスを設定してください
  </div>
{:else}
  <div class="overflow-auto h-full">
    <table class="border-collapse text-[11px] w-full">
      <tbody>
        <!-- ヘッダ行（データ行と同じスタイル） -->
        <tr class="hover:bg-primary/5">
          {#each data.headers as _, j}
            <td class="border border-gray-200 p-0 relative group/col">
              <input
                type="text"
                bind:value={data.headers[j]}
                class="w-full px-2 py-0.5 bg-transparent focus:outline-none focus:ring-1 focus:ring-inset focus:ring-primary"
              />
              <button
                type="button"
                onclick={() => deleteColumn(j)}
                class="absolute right-0.5 top-1/2 -translate-y-1/2
                       opacity-0 group-hover/col:opacity-100
                       text-gray-300 hover:text-red-400 text-[10px] leading-none px-0.5"
                title="列を削除"
              >
                ×
              </button>
            </td>
          {/each}
          <td class="border border-gray-200 w-6"></td>
        </tr>

        <!-- データ行 -->
        {#each data.rows as row, i}
          <tr class="hover:bg-primary/5">
            {#each row as _, j}
              <td class="border border-gray-200 p-0">
                <input
                  type="text"
                  bind:value={data.rows[i][j]}
                  class="w-full px-2 py-0.5 bg-transparent focus:outline-none focus:ring-1 focus:ring-inset focus:ring-primary"
                />
              </td>
            {/each}
            <td class="border border-gray-200 text-center">
              <button
                type="button"
                onclick={() => deleteRow(i)}
                class="text-gray-300 hover:text-red-400 px-1 leading-none"
                title="行を削除"
              >
                ×
              </button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>

    <div class="mt-2 flex gap-2">
      <button
        type="button"
        onclick={addRow}
        class="px-3 py-1 text-[11px] border border-dashed border-gray-300
               text-gray-400 hover:text-primary hover:border-primary rounded transition-colors"
      >
        ＋ 行を追加
      </button>
      <button
        type="button"
        onclick={addColumn}
        class="px-3 py-1 text-[11px] border border-dashed border-gray-300
               text-gray-400 hover:text-primary hover:border-primary rounded transition-colors"
      >
        ＋ 列を追加
      </button>
    </div>
  </div>
{/if}
