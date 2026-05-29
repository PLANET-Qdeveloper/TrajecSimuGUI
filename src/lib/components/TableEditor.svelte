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
</script>

{#if data.headers.length === 0}
  <div class="text-[11px] text-gray-400 italic py-4 text-center">
    データ未読込 — パラメータタブでCSVファイルパスを設定してください
  </div>
{:else}
  <div class="overflow-auto h-full">
    <table class="border-collapse text-[11px] w-full">
      <thead>
        <tr class="bg-gray-100 sticky top-0 z-10">
          {#each data.headers as header}
            <th
              class="border border-gray-200 px-2 py-1 text-left font-semibold text-gray-600 whitespace-nowrap"
            >
              {header}
            </th>
          {/each}
          <th class="border border-gray-200 w-6"></th>
        </tr>
      </thead>
      <tbody>
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

    <button
      type="button"
      onclick={addRow}
      class="mt-2 px-3 py-1 text-[11px] border border-dashed border-gray-300
             text-gray-400 hover:text-primary hover:border-primary rounded transition-colors"
    >
      + 行を追加
    </button>
  </div>
{/if}
