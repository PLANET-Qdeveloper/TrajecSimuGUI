<script lang="ts">
    import Chart from "$lib/components/Chart.svelte";
    import Select from "$lib/components/Select.svelte";
    import Input from "$lib/components/Input.svelte";
    import {
        channelStates,
        monitorStatus,
        rangeOptions,
        filterOptions,
    } from "$lib/stores/app";

    interface Props {
        title: string;
        chartId: string;
        channelIndex: number;
    }

    let { title, chartId, channelIndex }: Props = $props();

    // Store経由でチャネルの状態にアクセス
    function updateChannelState(
        key: "selectedRange" | "selectedFilter" | "calibrationCoefficient",
        value: string,
    ) {
        channelStates.update((states) => {
            states[channelIndex] = { ...states[channelIndex], [key]: value };
            return states;
        });
    }
</script>

<div class="p-4 flex flex-col h-full">
    <div class="flex-1 min-h-0">
        <Chart {title} {chartId} chartIndex={channelIndex} />
    </div>
    <div class="grid grid-cols-2 gap-2 mt-1.5">
        <Select
            options={rangeOptions}
            placeholder="レンジ"
            direction="up"
            disabled={!["idle", "not_connected"].includes($monitorStatus)}
            value={$channelStates[channelIndex].selectedRange}
            onchange={(val) => updateChannelState("selectedRange", val)}
        />
        <Select
            options={filterOptions}
            placeholder="フィルタ"
            direction="up"
            disabled={!["idle", "not_connected"].includes($monitorStatus)}
            value={$channelStates[channelIndex].selectedFilter}
            onchange={(val) => updateChannelState("selectedFilter", val)}
        />
        <Input
            class="col-span-2"
            placeholder="校正係数"
            disabled={!["idle", "not_connected"].includes($monitorStatus)}
            value={$channelStates[channelIndex].calibrationCoefficient}
            oninput={(e) =>
                updateChannelState(
                    "calibrationCoefficient",
                    (e.target as HTMLInputElement).value,
                )}
        />
    </div>
</div>
