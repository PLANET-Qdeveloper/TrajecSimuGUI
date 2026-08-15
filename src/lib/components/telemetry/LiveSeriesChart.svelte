<script module lang="ts">
  import type { DashStyleValue } from "highcharts";

  export interface SeriesSpec {
    id: string;
    name: string;
    color: string;
    dashStyle?: DashStyleValue;
    data: [number, number][];
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import Highcharts from "highcharts";

  interface Props {
    series: SeriesSpec[];
    yAxisTitle?: string;
    xAxisTitle?: string;
    visible?: boolean;
  }

  let {
    series = [],
    yAxisTitle = "",
    xAxisTitle = "経過時間 (s)",
    visible = true,
  }: Props = $props();

  let containerEl: HTMLDivElement;
  let chart = $state<Highcharts.Chart | undefined>();

  function seriesOptions(spec: SeriesSpec): Highcharts.SeriesLineOptions {
    return {
      type: "line",
      id: spec.id,
      name: spec.name,
      data: spec.data,
      color: spec.color,
      dashStyle: spec.dashStyle,
      lineWidth: 1,
      marker: { enabled: false },
      animation: false,
    };
  }

  onMount(() => {
    const c = Highcharts.chart(containerEl, {
      chart: {
        animation: false,
        zooming: { type: "xy" },
        style: { fontFamily: "inherit" },
      },
      title: { text: undefined },
      xAxis: { type: "linear", title: { text: xAxisTitle } },
      yAxis: { title: { text: yAxisTitle } },
      legend: { enabled: true },
      credits: { enabled: false },
      series: [],
    });
    chart = c;

    const observer = new ResizeObserver(() => c.reflow());
    observer.observe(containerEl);

    return () => {
      observer.disconnect();
      c.destroy();
      chart = undefined;
    };
  });

  $effect(() => {
    // ponytail: replace whole capped series; restore incremental updates only if profiling requires it.
    chart?.update({ series: series.map(seriesOptions) }, true, true, false);
  });

  $effect(() => {
    if (visible && chart) {
      requestAnimationFrame(() => chart?.reflow());
    }
  });
</script>

<div bind:this={containerEl} class="h-full w-full"></div>
