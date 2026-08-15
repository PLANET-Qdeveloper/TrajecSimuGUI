<script lang="ts">
  import {
    type SerialPortSlotConfig,
    type SerialPortPreset,
    type SerialFormat,
    dataKindOptions,
    defaultMainBoardByteMapping,
    defaultValveBoardCsvMapping,
    slotColor,
    DEFAULT_MAIN_DESTINATION_ID,
    DEFAULT_MAIN_SOURCE_ID,
  } from "$lib/types/telemetry";
  import Select from "$lib/components/Select.svelte";
  import Input from "$lib/components/Input.svelte";
  import NumberInput from "$lib/components/NumberInput.svelte";

  interface Props {
    slot: SerialPortSlotConfig;
    colorIndex: number;
    portOptions: string[];
    connected: boolean;
    onToggle: (enabled: boolean) => void;
  }

  let {
    slot = $bindable(),
    colorIndex,
    portOptions,
    connected,
    onToggle,
  }: Props = $props();

  const presetOptions: { value: SerialPortPreset; label: string }[] = [
    { value: "main", label: "メイン基板 既定" },
    { value: "valve", label: "バルブ基板 既定" },
    { value: "custom", label: "カスタム" },
  ];

  const formatOptions: { value: SerialFormat; label: string }[] = [
    { value: "byte", label: "バイト (TLVフレーム)" },
    { value: "csv", label: "CSV" },
  ];

  const pressureUnitOptions = [
    { value: "pa", label: "Pa" },
    { value: "hpa", label: "hPa" },
    { value: "kpa", label: "kPa" },
  ];

  const portSelectOptions = $derived([
    ...portOptions.map((p) => ({ value: p, label: p })),
    ...(slot.portPath && !portOptions.includes(slot.portPath)
      ? [{ value: slot.portPath, label: `${slot.portPath} (手入力)` }]
      : []),
  ]);

  function applyPreset(preset: SerialPortPreset) {
    slot.preset = preset;
    if (preset === "main") {
      slot.format = "byte";
      slot.byteMapping = defaultMainBoardByteMapping();
      slot.destinationId = DEFAULT_MAIN_DESTINATION_ID;
      slot.sourceId = DEFAULT_MAIN_SOURCE_ID;
    } else if (preset === "valve") {
      slot.format = "csv";
      slot.csvMapping = defaultValveBoardCsvMapping();
    }
  }

  function addByteRow() {
    slot.byteMapping = [
      ...slot.byteMapping,
      { tag: "", kind: slot.byteMapping[0]?.kind },
    ];
  }
  function removeByteRow(i: number) {
    slot.byteMapping = slot.byteMapping.filter((_, j) => j !== i);
  }
  function addCsvRow() {
    slot.csvMapping = [
      ...slot.csvMapping,
      { index: slot.csvMapping.length, kind: slot.csvMapping[0]?.kind },
    ];
  }
  function removeCsvRow(i: number) {
    slot.csvMapping = slot.csvMapping.filter((_, j) => j !== i);
  }
</script>

<div class="border overflow-hidden flex flex-col">
  <div
    class="flex items-center gap-2 px-2 py-1.5 border-b bg-gray-50"
    style="border-left: 4px solid {slotColor(colorIndex)}"
  >
    <label
      class="flex items-center gap-1.5 cursor-pointer select-none shrink-0"
    >
      <input
        type="checkbox"
        class="accent-primary"
        checked={slot.enabled}
        onchange={(e) => onToggle((e.target as HTMLInputElement).checked)}
      />
      <span
        class="w-2 h-2 rounded-full inline-block"
        class:bg-green-500={connected}
        class:bg-gray-300={!connected}
      ></span>
    </label>
    <Input bind:value={slot.label} class="flex-1 min-w-0 font-medium" />
  </div>

  <div class="p-2 flex flex-col gap-2 text-xs overflow-y-auto">
    <div class="grid grid-cols-2 gap-2">
      <div>
        <div class="text-[10px] text-gray-500 mb-0.5">ポート</div>
        <Select
          options={portSelectOptions}
          bind:value={slot.portPath}
          placeholder="ポートを選択 or 入力"
        />
        <Input
          bind:value={slot.portPath}
          placeholder="/dev/ttys001 など手入力"
          class="mt-1"
        />
      </div>
      <div>
        <div class="text-[10px] text-gray-500 mb-0.5">通信速度 (baud)</div>
        <NumberInput bind:value={slot.baudRate} step={1} min={1} />
      </div>
      <div>
        <div class="text-[10px] text-gray-500 mb-0.5">フォーマット</div>
        <Select
          options={formatOptions}
          value={slot.format}
          onchange={(v) => (slot.format = v as SerialFormat)}
        />
      </div>
      <div>
        <div class="text-[10px] text-gray-500 mb-0.5">プリセット</div>
        <Select
          options={presetOptions}
          value={slot.preset}
          onchange={(v) => applyPreset(v as SerialPortPreset)}
        />
      </div>
    </div>

    <label
      class="flex items-center gap-1.5 cursor-pointer select-none"
      class:opacity-50={slot.enabled}
    >
      <input
        type="checkbox"
        class="accent-primary"
        bind:checked={slot.record}
        disabled={slot.enabled}
      />
      <span
        >受信データをDownloadsフォルダへ保存する（生バイト列+デコード済みCSV）</span
      >
    </label>
    {#if slot.enabled}
      <div class="text-[10px] text-gray-400 -mt-1.5">
        接続中は変更できません。切断後に変更してください。
      </div>
    {/if}

    <div class="grid grid-cols-2 gap-2">
      <div>
        <div class="text-[10px] text-gray-500 mb-0.5">
          気圧単位 ({slot.format === "csv"
            ? "バルブ基板の生気圧値、仕様上はkPa"
            : "メイン基板の生気圧値、仕様上単位未規定"})
        </div>
        <Select
          options={pressureUnitOptions}
          bind:value={slot.pressureUnit}
          class="w-24"
        />
      </div>
      <div>
        <div class="text-[10px] text-gray-500 mb-0.5">
          海面基準気圧 (常にPa)
        </div>
        <NumberInput bind:value={slot.seaLevelPressurePa} step={10} />
      </div>
    </div>

    <div class="border-t pt-1.5">
      <div class="flex items-center justify-between mb-1">
        <span
          class="text-[10px] font-semibold text-gray-500 uppercase tracking-wider"
        >
          データ割当
        </span>
        <button
          type="button"
          class="text-[10px] text-primary hover:underline"
          onclick={slot.format === "byte" ? addByteRow : addCsvRow}
        >
          + 追加
        </button>
      </div>

      {#if slot.format === "byte"}
        <div class="grid grid-cols-2 gap-2 mb-1.5">
          <div>
            <div class="text-[10px] text-gray-500 mb-0.5">宛先ID (16進)</div>
            <Input bind:value={slot.destinationId} placeholder="0B" />
          </div>
          <div>
            <div class="text-[10px] text-gray-500 mb-0.5">送信元ID (16進)</div>
            <Input bind:value={slot.sourceId} placeholder="0A" />
          </div>
        </div>
        <table class="w-full text-[11px]">
          <thead>
            <tr class="text-gray-500">
              <th class="text-left font-normal w-16">Tag</th>
              <th class="text-left font-normal">種類</th>
              <th class="w-6"></th>
            </tr>
          </thead>
          <tbody>
            {#each slot.byteMapping as row, i (i)}
              <tr>
                <td class="pr-1 py-0.5">
                  <Input bind:value={row.tag} placeholder="A1" />
                </td>
                <td class="py-0.5">
                  <Select options={dataKindOptions} bind:value={row.kind} />
                </td>
                <td class="text-center">
                  <button
                    type="button"
                    class="text-gray-400 hover:text-red-500"
                    onclick={() => removeByteRow(i)}>×</button
                  >
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <table class="w-full text-[11px]">
          <thead>
            <tr class="text-gray-500">
              <th class="text-left font-normal w-12">列 #</th>
              <th class="text-left font-normal">種類</th>
              <th class="w-6"></th>
            </tr>
          </thead>
          <tbody>
            {#each slot.csvMapping as row, i (i)}
              <tr>
                <td class="pr-1 py-0.5">
                  <NumberInput bind:value={row.index} step={1} min={0} />
                </td>
                <td class="py-0.5">
                  <Select options={dataKindOptions} bind:value={row.kind} />
                </td>
                <td class="text-center">
                  <button
                    type="button"
                    class="text-gray-400 hover:text-red-500"
                    onclick={() => removeCsvRow(i)}>×</button
                  >
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>
</div>
