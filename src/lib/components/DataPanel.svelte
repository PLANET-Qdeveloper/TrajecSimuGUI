<script lang="ts">
    import ChannelPanel from "$lib/components/ChannelPanel.svelte";
    import { channelStates, monitorStatus } from "$lib/stores/app";

    const CHANNELS = [
        { title: "Channel 1", chartId: "channel1" },
        { title: "Channel 2", chartId: "channel2" },
        { title: "Channel 3", chartId: "channel3" },
        { title: "Channel 4", chartId: "channel4" },
    ];

    let activeIndices = $derived(
        CHANNELS.map((_, i) => i).filter((i) => $channelStates[i].isActive),
    );
    let activeCount = $derived(activeIndices.length);

    function toggleChannel(i: number) {
        channelStates.update((states) => {
            states[i] = { ...states[i], isActive: !states[i].isActive };
            return states;
        });
    }

    let canToggle = $derived(
        ["idle", "not_connected"].includes($monitorStatus),
    );
</script>

<div class="flex-1 flex flex-col overflow-hidden">
    <div class="flex-1 min-h-0">
        {#if activeCount <= 1}
            {@const idx = activeIndices[0] ?? 0}
            <div class="h-full">
                <ChannelPanel
                    title={CHANNELS[idx].title}
                    chartId={CHANNELS[idx].chartId}
                    channelIndex={idx}
                />
            </div>
        {:else if activeCount === 2}
            <div class="grid grid-rows-2 h-full">
                {#each activeIndices as idx, j}
                    <div
                        class="{j === 0 ? 'border-b' : 'border-t'} border-gray"
                    >
                        <ChannelPanel
                            title={CHANNELS[idx].title}
                            chartId={CHANNELS[idx].chartId}
                            channelIndex={idx}
                        />
                    </div>
                {/each}
            </div>
        {:else}
            <div class="grid grid-cols-2 grid-rows-2 h-full">
                {#each CHANNELS as ch, i}
                    <div
                        class="relative {i % 2 === 0
                            ? 'border-r'
                            : 'border-l'} {i < 2
                            ? 'border-b'
                            : 'border-t'} border-gray"
                    >
                        <ChannelPanel
                            title={ch.title}
                            chartId={ch.chartId}
                            channelIndex={i}
                        />
                        {#if !$channelStates[i].isActive}
                            <div
                                class="absolute inset-0 bg-white/60 pointer-events-none"
                            ></div>
                        {/if}
                    </div>
                {/each}
            </div>
        {/if}
    </div>

    <!-- CH1〜4トグルボタン（右下） -->
    <div class="flex justify-end gap-1 px-2 py-1 shrink-0">
        {#each CHANNELS as _ch, i}
            <button
                onclick={() => toggleChannel(i)}
                disabled={!canToggle}
                class="px-2 py-0.5 text-xs rounded border transition-colors
                    {$channelStates[i].isActive
                        ? 'bg-primary text-white border-primary'
                        : 'bg-white text-black border-gray'}
                    disabled:opacity-40 disabled:cursor-not-allowed"
            >
                CH{i + 1}
            </button>
        {/each}
    </div>
</div>
