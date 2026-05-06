<script lang="ts">
    import Highcharts from "highcharts";
    import "highcharts/modules/boost";
    import { chartData } from "$lib/stores/app";

    interface Props {
        title?: string;
        color?: string;
        chartId?: string;
        chartIndex?: number;
    }

    let {
        title = "Sample Chart",
        color = "#000000",
        chartId = "chart0",
        chartIndex = 0,
    }: Props = $props();

    let containerEl: HTMLDivElement;
    // $state にして2つ目の $effect がリアクティブに追跡できるようにする
    let chart = $state<Highcharts.Chart | undefined>();

    // チャートの初期化
    $effect(() => {
        if (!containerEl) return;

        chart = Highcharts.chart(containerEl, {
            chart: {
                animation: false,
                zooming: { type: "xy" },
            },
            boost: {
                useGPUTranslations: true,
                usePreallocated: true,
                seriesThreshold: 1,
            },
            title: {
                text: title,
                align: "left",
                style: { fontSize: "16px" },
            },
            xAxis: { type: "linear" },
            yAxis: { title: { text: "value" } },
            legend: { enabled: false },
            credits: { enabled: false },
            series: [
                {
                    type: "line",
                    data: [],
                    lineWidth: 0.5,
                    color: color,
                    turboThreshold: 0,
                    cropThreshold: Infinity, // 表示領域外の点も保持
                    boostThreshold: 0, // 常にboostレンダリングを使用
                    animation: false,
                },
            ],
        });

        const observer = new ResizeObserver(() => chart?.reflow());
        observer.observe(containerEl);

        return () => {
            observer.disconnect();
            chart?.destroy();
            chart = undefined;
        };
    });

    // データ更新
    $effect(() => {
        const data = $chartData[chartIndex] ?? [];
        if (!chart?.series[0]) return;
        chart.series[0].setData(
            data as Highcharts.PointOptionsType[],
            true,
            false,
            false,
        );
        chart.redraw(); // boost使用時は明示的に redraw
    });
</script>

<div bind:this={containerEl} class="h-full w-full overflow-hidden"></div>
