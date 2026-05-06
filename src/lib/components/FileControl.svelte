<script lang="ts">
    import { fileList } from "$lib/stores/app";
    import { format } from "d3-format";
    import * as commands from "$lib/commands";
    import { message } from "@tauri-apps/plugin-dialog";

    let selectedIndex: number | null = $state(null);
    let menuInfo = $state<{
        visible: boolean;
        x: number;
        y: number;
        index: number | null;
    }>({ visible: false, x: 0, y: 0, index: null });

    function select(index: number) {
        if (selectedIndex === index) {
            selectedIndex = null;
        } else {
            selectedIndex = index;
        }
    }

    function handleClick(event: MouseEvent, index: number) {
        if (disable) return;
        event.stopPropagation();

        menuInfo.visible = false;
    }

    function handleContextMenu(event: MouseEvent, index: number) {
        event.preventDefault();
        event.stopPropagation();
        if (disable) return;
        selectedIndex = index;
        menuInfo = {
            visible: true,
            x: event.clientX,
            y: event.clientY,
            index: index,
        };
    }

    function closeMenu() {
        selectedIndex = null;
        menuInfo.visible = false;
        menuInfo.index = null;
    }


    interface Props {
        disable?: boolean;
    }

    function tableBackground(index: number) {
        if (disable) {
            return "bg-gray";
        } else if (selectedIndex === index) {
            return "bg-primary text-white";
        } else {
            return "bg-white hover:bg-primary hover:text-white";
        }
    }

    let { disable = false }: Props = $props();
</script>

<svelte:window onclick={closeMenu} />
<div
    class="overflow-y-auto w-full h-full {disable
        ? 'opacity-60 bg-gray'
        : 'bg-white'}"
>
    <table class="w-full border-separate border-spacing-0">
        <thead class={disable ? "pointer-events-none bg-gray" : "bg-white"}>
            <tr>
                <th
                    class="sticky {disable
                        ? 'bg-gray'
                        : 'bg-white'} top-0 text-left px-3 py-2 border-b border-gray"
                >
                    ファイル名
                </th>
                <th
                    class="sticky {disable
                        ? 'bg-gray'
                        : 'bg-white'} top-0 text-left px-3 py-2 border-b border-gray"
                >
                    サイズ
                </th>
                <th
                    class="sticky {disable
                        ? 'bg-gray'
                        : 'bg-white'} top-0 text-left px-3 py-2 border-b border-gray"
                >
                    日付
                </th>
            </tr>
        </thead>
        <tbody>
            {#each $fileList as file, index}
                <tr
                    class="{disable
                        ? 'cursor-not-allowed pointer-events-none'
                        : 'cursor-pointer'}
                        select-none
                        {tableBackground(index)}"
                    onclick={(event) => handleClick(event, index)}
                    oncontextmenu={(event) => handleContextMenu(event, index)}
                >
                    <td class="font-medium px-4 py-2"
                        >{file.name.slice(0, 9) +
                            (file.name.length > 9 ? "..." : "")}</td
                    >
                    <td class="text-sm px-4 py-2">
                        {format(".2s")(file.size)}
                    </td>
                    <td class="text-sm px-4 py-2">
                        {file.date.toLocaleDateString()}
                    </td>
                </tr>
            {/each}
        </tbody>
    </table>
</div>

{#if menuInfo.visible}
    <div
        class="fixed bg-white z-50 border shadow-xl py-1 w-32 overflow-hidden"
        style="top: {menuInfo.y}px; left: {menuInfo.x}px;"
    >
        <button
            class="block w-full text-left px-4 py-2 text-sm text-black hover:bg-primary hover:text-white transition-colors"
            onclick={async () => {
                if (menuInfo.index === null) return;
                const file = $fileList[menuInfo.index];
                closeMenu();
                await commands.downloadFile(file.name, file.size).catch((e) =>
                    message(`ダウンロードに失敗しました: ${e}`, { kind: "error" }),
                );
            }}
        >
            ダウンロード
        </button>
        <div class="border-t my-1"></div>
        <button
            class="block w-full text-left px-4 py-2 text-sm text-error hover:bg-primary hover:text-white transition-colors"
            onclick={async () => {
                if (menuInfo.index === null) return;
                const filename = $fileList[menuInfo.index].name;
                await commands.deleteFile(filename).catch((e) =>
                    message(`削除に失敗しました: ${e}`, { kind: "error" }),
                );
                closeMenu();
            }}
        >
            削除する
        </button>
    </div>
{/if}
