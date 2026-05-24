<script lang="ts">
  import { onMount } from "svelte";
  import Highcharts from "highcharts";
  import "highcharts/modules/boost";
  import { TelemetryDataKey, type Trajectory } from "$lib/types/config";
  import Select from "$lib/components/Select.svelte";

  interface Props {
    trajectory_ballistic: Trajectory | null;
    trajectory_parachute: Trajectory | null;
    visible?: boolean;
  }

  let {
    trajectory_ballistic = null,
    trajectory_parachute = null,
    visible = true,
  }: Props = $props();

  const y1Color = "#d55e00";
  const y2Color = "#cc79a7";

  const keyLabel: Record<string, string> = {
    [TelemetryDataKey.AltMslM]: "Alt MSL (m)",
    [TelemetryDataKey.DownRangeM]: "Down Range (m)",
    [TelemetryDataKey.LocalXM]: "Local X (m)",
    [TelemetryDataKey.LocalYM]: "Local Y (m)",
    [TelemetryDataKey.LatDeg]: "Lat (°)",
    [TelemetryDataKey.LonDeg]: "Lon (°)",
    [TelemetryDataKey.TrueAirspeedMps]: "Airspeed (m/s)",
    [TelemetryDataKey.GroundSpeedMps]: "Gnd Speed (m/s)",
    [TelemetryDataKey.UMps]: "u (m/s)",
    [TelemetryDataKey.VMps]: "v (m/s)",
    [TelemetryDataKey.WMps]: "w (m/s)",
    [TelemetryDataKey.PitchDeg]: "Pitch (°)",
    [TelemetryDataKey.RollDeg]: "Roll (°)",
    [TelemetryDataKey.YawDeg]: "Yaw (°)",
    [TelemetryDataKey.PRadSec]: "p (rad/s)",
    [TelemetryDataKey.QRadSec]: "q (rad/s)",
    [TelemetryDataKey.RRadSec]: "r (rad/s)",
    [TelemetryDataKey.AxMps2]: "Ax (m/s²)",
    [TelemetryDataKey.AyMps2]: "Ay (m/s²)",
    [TelemetryDataKey.AzMps2]: "Az (m/s²)",
    [TelemetryDataKey.AlphaDeg]: "α (°)",
    [TelemetryDataKey.BetaDeg]: "β (°)",
    [TelemetryDataKey.QbarPa]: "q̄ (Pa)",
    [TelemetryDataKey.TotalAoaDeg]: "AoA (°)",
    [TelemetryDataKey.PressurePa]: "P (Pa)",
    [TelemetryDataKey.TemperatureK]: "T (K)",
    [TelemetryDataKey.GustAirspeedMps]: "Gust V (m/s)",
    [TelemetryDataKey.GustAoaDeg]: "Gust AoA (°)",
    [TelemetryDataKey.ThrustN]: "Thrust (N)",
    [TelemetryDataKey.Mach]: "Mach",
  };

  const phaseOptions = [
    { value: "ballistic", label: "弾道" },
    { value: "parachute", label: "落下傘" },
  ];

  const keyOptions = Object.entries(keyLabel).map(([value, label]) => ({
    value,
    label,
  }));

  let selectedPhase = $state("ballistic");
  let selectedY1 = $state<string>(TelemetryDataKey.AltMslM);
  let selectedY2 = $state<string>(TelemetryDataKey.TrueAirspeedMps);

  let containerEl: HTMLDivElement;
  let chart = $state<Highcharts.Chart | undefined>();

  onMount(() => {
    const c = Highcharts.chart(containerEl, {
      chart: {
        animation: false,
        zooming: { type: "xy" },
        style: { fontFamily: "inherit" },
      },
      boost: {
        useGPUTranslations: true,
        usePreallocated: true,
        seriesThreshold: 1,
      },
      title: { text: undefined },
      xAxis: {
        type: "linear",
        title: { text: "Time (s)" },
      },
      yAxis: [
        {
          title: { text: "Y1", style: { color: y1Color } },
          labels: { style: { color: y1Color } },
          opposite: false,
        },
        {
          title: { text: "Y2", style: { color: y2Color } },
          labels: { style: { color: y2Color } },
          opposite: true,
        },
      ],
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
    const c = chart;
    if (!c) return;

    const traj =
      selectedPhase === "ballistic"
        ? trajectory_ballistic
        : trajectory_parachute;
    const y1 = selectedY1;
    const y2 = selectedY2;

    const newSeries: Highcharts.SeriesOptionsType[] = [];

    if (traj) {
      const time = traj.time_sec;
      const rec = traj as unknown as Record<string, number[]>;

      newSeries.push({
        type: "line",
        name: keyLabel[y1],
        data: time.map((t, j) => [t, rec[y1][j]] as [number, number]),
        yAxis: 0,
        color: y1Color,
        lineWidth: 1,
        marker: { enabled: false },
        turboThreshold: 0,
        boostThreshold: 0,
        animation: false,
      });

      newSeries.push({
        type: "line",
        name: keyLabel[y2],
        data: time.map((t, j) => [t, rec[y2][j]] as [number, number]),
        yAxis: 1,
        color: y2Color,
        lineWidth: 1,
        marker: { enabled: false },
        turboThreshold: 0,
        boostThreshold: 0,
        animation: false,
      });
    }

    c.update(
      {
        yAxis: [
          { title: { text: keyLabel[y1] ?? "Y1" } },
          { title: { text: keyLabel[y2] ?? "Y2" } },
        ],
        series: newSeries,
      },
      true,
      true,
    );
  });

  $effect(() => {
    if (visible && chart) {
      requestAnimationFrame(() => chart?.reflow());
    }
  });
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Top selector bar -->
  <div
    class="flex items-center gap-4 px-3 py-1.5 border-b bg-gray-50 shrink-0 text-xs"
  >
    <div class="flex items-center gap-1.5">
      <span
        class="font-semibold text-gray-500 uppercase tracking-wider text-[10px] whitespace-nowrap"
        >フェーズ</span
      >
      <Select options={phaseOptions} bind:value={selectedPhase} class="w-24" />
    </div>
    <div class="flex items-center gap-1.5">
      <span
        class="font-semibold text-[10px] uppercase tracking-wider whitespace-nowrap"
        style="color:{y1Color}">Y1 (左軸)</span
      >
      <Select options={keyOptions} bind:value={selectedY1} class="w-36" />
    </div>
    <div class="flex items-center gap-1.5">
      <span
        class="font-semibold text-[10px] uppercase tracking-wider whitespace-nowrap"
        style="color:{y2Color}">Y2 (右軸)</span
      >
      <Select options={keyOptions} bind:value={selectedY2} class="w-36" />
    </div>
  </div>
  <!-- Chart -->
  <div class="flex-1 min-h-0">
    <div bind:this={containerEl} class="h-full w-full"></div>
  </div>
</div>
