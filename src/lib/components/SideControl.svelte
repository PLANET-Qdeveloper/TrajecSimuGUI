<!-- ControlPanel.svelte -->
<script lang="ts">
    import { onMount } from "svelte";
    import { animate } from "animejs";
    import Button from "$lib/components/Button.svelte";
    import Select from "$lib/components/Select.svelte";
    import Input from "$lib/components/Input.svelte";
    import Start from "$lib/components/icons/Start.svelte";
    import Stop from "$lib/components/icons/Stop.svelte";
    import FileControl from "$lib/components/FileControl.svelte";
    import {
        ipAddress,
        subnetMask,
        ethernetInterface,
        recordingFilename,
        frequency,
        frequencyOptions,
        monitorStatus,
        storageFreeSpace,
        saveDataPath,
    } from "$lib/stores/app";
    import * as commands from "$lib/commands";
    import { message } from "@tauri-apps/plugin-dialog";

    const ipv4Pattern =
        /^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/;

    let ipError = $state("");

    function validate_ip_address(_: Event) {
        if (!$ipAddress) {
            ipError = "IPアドレスを入力してください";
        } else if (!ipv4Pattern.test($ipAddress)) {
            ipError = "正しいIPv4形式で入力してください";
        } else {
            ipError = "";
        }
    }

    function validate_subnet_mask(_: Event) {
        if (!$subnetMask) {
            ipError = "サブネットマスクを入力してください";
        } else if (!ipv4Pattern.test($subnetMask)) {
            ipError = "正しいサブネット形式で入力してください";
        } else {
            ipError = "";
        }
    }

    type EthernetOption = { label: string; value: string };
    let ethernetOptions = $state<EthernetOption[]>([]);

    function check_ethernet_interface() {
        commands
            .fetchNetworkInterfaces()
            .then((result) => {
                ethernetOptions = result.map((iface) => ({
                    label:
                        iface.name +
                        (iface.description ? " - " + iface.description : "") +
                        (iface.is_configured ? " (設定済み)" : ""),
                    value: iface.name,
                }));
            })
            .catch((e) =>
                message(
                    `ネットワークインターフェースの取得に失敗しました: ${e}`,
                    { kind: "error" },
                ),
            );
    }

    onMount(() => {
        commands.initDefaults();
        check_ethernet_interface();
    });

    let recordingFilenameError = $state("");

    function validateRecordingFilename(_: Event) {
        if (!$recordingFilename) {
            recordingFilenameError = "ファイル名を入力してください";
        } else if (!/^[\w,\s-]+(\.[A-Za-z]{1,4})?$/.test($recordingFilename)) {
            recordingFilenameError = "有効なファイル名を入力してください";
        } else if ($recordingFilename.length > 20) {
            recordingFilenameError = "ファイル名が長すぎます（最大20文字）";
        } else {
            recordingFilenameError = "";
        }
    }

    let connectError = $state("");

    // --- 録音中メトロノーム (Web Audio API ルックアヘッドスケジューラ) ---
    let audioCtx: AudioContext | null = null;
    let audioBuffer: AudioBuffer | null = null;
    let metronomeInterval: ReturnType<typeof setInterval> | null = null;
    let nextBeatTime = 0;

    async function ensureAudioReady() {
        if (!audioCtx) audioCtx = new AudioContext();
        if (audioCtx.state === "suspended") await audioCtx.resume();
        if (!audioBuffer) {
            const res = await fetch("/audio/BeMyBaby.wav");
            const buf = await res.arrayBuffer();
            audioBuffer = await audioCtx.decodeAudioData(buf);
        }
    }

    function scheduleBeat() {
        if (!audioCtx || !audioBuffer) return;
        const LOOK_AHEAD = 0.1; // 100ms 先読み
        while (nextBeatTime < audioCtx.currentTime + LOOK_AHEAD) {
            const src = audioCtx.createBufferSource();
            src.buffer = audioBuffer;
            src.connect(audioCtx.destination);
            src.start(nextBeatTime);
            nextBeatTime += 1.0; // 正確に1秒間隔
        }
    }

    async function startMetronome() {
        await ensureAudioReady();
        nextBeatTime = audioCtx!.currentTime;
        metronomeInterval = setInterval(scheduleBeat, 25); // 25ms ごとにチェック
    }

    function stopMetronome() {
        if (metronomeInterval) {
            clearInterval(metronomeInterval);
            metronomeInterval = null;
        }
    }

    $effect(() => {
        if ($monitorStatus === "recording") {
            startMetronome().catch((e) =>
                message(`メトロノームの開始に失敗しました: ${e}`, {
                    kind: "error",
                }),
            );
        } else {
            stopMetronome();
        }
        return () => stopMetronome();
    });

    let freeSpaceUsageValue = $state(0);

    $effect(() => {
        if ($monitorStatus === "processing") return;
        const targetValue =
            $monitorStatus === "not_connected" ? 0 : 100 - $storageFreeSpace;

        const proxy = { value: freeSpaceUsageValue };

        const distance = Math.abs(targetValue - freeSpaceUsageValue);

        const calculatedDuration = distance === 0 ? 0 : (distance / 25) * 1000;

        if (calculatedDuration > 0) {
            animate(proxy, {
                value: targetValue,
                duration: calculatedDuration,
                easing: "inOut",
                onUpdate: () => {
                    freeSpaceUsageValue = Math.round(proxy.value);
                },
            });
        }
    });

    async function connect() {
        connectError = "";
        try {
            await commands.connect();
        } catch (e) {
            connectError = String(e);
            await message(String(e), { kind: "error" });
        } finally {
            check_ethernet_interface();
        }
    }

    async function disconnect() {
        try {
            await commands.disconnect();
        } finally {
            check_ethernet_interface();
        }
    }
</script>

<div
    class="px-4 pt-2 flex flex-col gap-5 h-full overflow-y-auto border-gray border-l-2"
>
    <!-- 接続セクション -->
    <section class="flex flex-col gap-1">
        <h2 class="text-sm mb-2">接続設定</h2>

        <div class="flex gap-1">
            <Select
                class="flex-1 h-9.5"
                options={ethernetOptions}
                placeholder="イーサネットを選択"
                disabled={$monitorStatus === "processing" ||
                    $monitorStatus !== "not_connected"}
                value={$ethernetInterface}
                onchange={(v) => ethernetInterface.set(v)}
                onpush={check_ethernet_interface}
            />

            <Button
                onclick={$monitorStatus === "not_connected"
                    ? connect
                    : disconnect}
                variant="primary"
                class="flex-none"
                disabled={$monitorStatus === "processing" ||
                    ($monitorStatus !== "not_connected" &&
                        $monitorStatus !== "idle")}
                >{$monitorStatus === "not_connected" ? "接続" : "切断"}
            </Button>
        </div>
    </section>

    <!-- 新規追加セクション -->
    <section>
        <h2 class="text-sm mb-2">操作</h2>
        <div class="flex flex-col gap-2">
            <div class="flex gap-1">
                <Button
                    onclick={commands.startMonitoring}
                    variant="primary"
                    class="w-full"
                    disabled={$monitorStatus === "processing" ||
                        $monitorStatus !== "idle"}
                >
                    <span class="align-middle">測定開始</span>
                    <Start
                        size="16"
                        class="inline-block ml-1 stroke-white align-middle"
                    />
                </Button>
                <Button
                    onclick={commands.stopMonitoring}
                    variant="primary"
                    class="w-full"
                    disabled={$monitorStatus === "processing" ||
                        $monitorStatus !== "monitoring"}
                >
                    <span class="align-middle">測定停止</span>
                    <Stop
                        size="16"
                        class="inline-block ml-1 stroke-white align-middle"
                    />
                </Button>
            </div>

            <Button
                onclick={commands.balance}
                variant="primary"
                class="w-full"
                disabled={$monitorStatus === "processing" ||
                    $monitorStatus !== "monitoring"}
            >
                バランス
            </Button>
            <div class="flex gap-1">
                <Button
                    onclick={commands.startRecording}
                    variant="primary"
                    class="w-full"
                    disabled={$monitorStatus === "processing" ||
                        $monitorStatus !== "monitoring"}
                >
                    データ保存開始
                </Button>
                <Button
                    onclick={commands.stopRecording}
                    variant="primary"
                    class="w-full"
                    disabled={$monitorStatus === "processing" ||
                        $monitorStatus !== "recording"}
                >
                    データ保存停止
                </Button>
            </div>
        </div>
    </section>

    <!-- 測定条件セクション -->
    <section>
        <h2 class="text-sm text-black font-medium mb-2">測定条件</h2>
        <div class="flex flex-col gap-2">
            <div class="flex items-center gap-1">
                <label
                    for="frequency"
                    class="text-sm my-2 mr-2 font-medium mb-2"
                >
                    測定周波数
                </label>
                <Select
                    class="flex-1 h-9.5"
                    options={frequencyOptions}
                    disabled={!["idle", "not_connected"].includes(
                        $monitorStatus,
                    )}
                    value={$frequency}
                    onchange={(v) => frequency.set(v)}
                />
            </div>
            <!--
            <div class="flex gap-1">
                <Button
                    onclick={commands.loadConditions}
                    variant="primary"
                    class="w-full"
                    disabled={$monitorStatus === "processing" ||
                        !["idle"].includes($monitorStatus)}
                >
                    <span class="align-middle">条件読み込み</span>
                </Button>
                <Button
                    onclick={commands.saveConditions}
                    variant="primary"
                    class="w-full"
                    disabled={$monitorStatus === "processing" ||
                        !["idle"].includes($monitorStatus)}
                >
                    <span class="align-middle">条件保存</span>
                </Button>
            </div>
            -->
        </div>
    </section>

    <section class="grow flex flex-col min-h-0">
        <h2 class="text-sm text-black font-medium mb-2 shrink-0">データ保存</h2>
        <div class="flex flex-col gap-2 shrink-0">
            <div class="flex gap-1">
                <Input
                    class="flex-1"
                    name="saveDataPath"
                    bind:value={$saveDataPath}
                    placeholder="保存先パス"
                    disabled={[
                        "recording",
                        "recording_paused",
                        "processing",
                    ].includes($monitorStatus)}
                />
                <Button
                    onclick={commands.selectSavePath}
                    variant="primary"
                    class="flex-none"
                    disabled={[
                        "recording",
                        "recording_paused",
                        "processing",
                    ].includes($monitorStatus)}
                >
                    …
                </Button>
            </div>
        </div>

        <div class="flex-1 min-h-40 overflow-hidden bg-white my-2">
            <FileControl disable={$monitorStatus !== "idle"} />
        </div>
        <div class="relative w-full h-6 overflow-hidden shrink-0 bg-white">
            <div
                class="h-full bg-primary flex items-center justify-end pr-2"
                style="width: {freeSpaceUsageValue}%;"
            >
                {#if freeSpaceUsageValue > 20}
                    <span class="text-xs text-white font-bold"
                        >{freeSpaceUsageValue}%</span
                    >
                {/if}
            </div>
            {#if freeSpaceUsageValue <= 20}
                <div
                    class="absolute top-0 h-full flex items-center pointer-events-none"
                    style="left: {freeSpaceUsageValue}%;"
                >
                    <span
                        class="text-xs text-black font-bold ml-2 whitespace-nowrap"
                    >
                        {freeSpaceUsageValue}%
                    </span>
                </div>
            {/if}
        </div>
        <div class="relative w-full h-2 overflow-hidden shrink-0"></div>
    </section>
</div>
