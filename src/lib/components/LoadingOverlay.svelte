<script>
    import { fade } from "svelte/transition";
    import { monitorStatus } from "$lib/stores/app";
    import Rocket from "$lib/components/icons/Rocket.svelte";
    import { progressInfo } from "$lib/stores/app";
</script>

{#if $monitorStatus === "processing"}
    <div
        transition:fade={{ duration: 200 }}
        class="fixed inset-0 z-9999 flex flex-col items-center justify-center bg-black/50"
    >
        <Rocket />
        <p class="mt-4 text-lg font-bold text-white">{$progressInfo.message}</p>
    </div>
{/if}
