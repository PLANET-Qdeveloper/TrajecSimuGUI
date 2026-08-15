<script lang="ts">
  import { onMount } from "svelte";
  import maplibregl from "maplibre-gl";
  import "maplibre-gl/dist/maplibre-gl.css";
  import { getTileBaseUrl } from "$lib/utils/tileBaseUrl";
  import { MapboxOverlay } from "@deck.gl/mapbox";
  import { ScatterplotLayer, PathLayer } from "@deck.gl/layers";
  import type { Layer } from "@deck.gl/core";

  interface Track {
    id: string;
    color: [number, number, number];
    points: [number, number, number][]; // [lon, lat, alt]
  }

  interface Props {
    tracks: Track[];
    visible?: boolean;
  }

  let { tracks = [], visible = true }: Props = $props();

  let mapContainer: HTMLDivElement;
  let map: maplibregl.Map | undefined;
  let overlay: MapboxOverlay | undefined;
  let mapLoaded = false;
  let hasFitBounds = false;
  let wasVisible = false;

  /** Frames the camera to the bounding box of every current track's points.
   * Returns whether it actually fit anything (false if there's no data yet). */
  function fitToTracks(): boolean {
    if (!map) return false;
    const allPoints = tracks.flatMap((t) => t.points);
    if (allPoints.length === 0) return false;
    const lngs = allPoints.map((p) => p[0]);
    const lats = allPoints.map((p) => p[1]);
    map.fitBounds(
      [
        [Math.min(...lngs), Math.min(...lats)],
        [Math.max(...lngs), Math.max(...lats)],
      ],
      { padding: 60, maxZoom: 15 },
    );
    return true;
  }

  function applyDeckLayers() {
    if (!overlay) return;

    const layers: Layer[] = [];
    for (const track of tracks) {
      if (track.points.length === 0) continue;
      layers.push(
        new PathLayer({
          id: `telemetry-path-${track.id}`,
          data: [{ path: track.points }],
          getPath: (d: { path: [number, number, number][] }) => d.path,
          getColor: [...track.color, 160],
          getWidth: 2,
          widthUnits: "pixels",
          pickable: false,
        }),
      );
      layers.push(
        new ScatterplotLayer({
          id: `telemetry-marker-${track.id}`,
          data: [track.points[track.points.length - 1]],
          getPosition: (d: [number, number, number]) => d,
          getRadius: 6,
          radiusUnits: "pixels",
          getFillColor: [...track.color, 255],
          stroked: true,
          getLineColor: [255, 255, 255, 255],
          lineWidthUnits: "pixels",
          getLineWidth: 1.5,
          pickable: false,
        }),
      );
    }
    overlay.setProps({ layers });

    if (!hasFitBounds && fitToTracks()) {
      hasFitBounds = true;
    }
  }

  $effect(() => {
    void tracks;
    if (mapLoaded) applyDeckLayers();
  });

  $effect(() => {
    if (visible && map) {
      requestAnimationFrame(() => map?.resize());
      // Re-frame the camera whenever this tab regains visibility (e.g. the
      // user switches away and back), not just the first time data ever
      // arrives — a manual pan/zoom while away shouldn't stick forever.
      if (!wasVisible) fitToTracks();
    }
    wasVisible = visible;
  });

  onMount(() => {
    const tileBase = getTileBaseUrl();
    map = new maplibregl.Map({
      container: mapContainer,
      style: {
        version: 8,
        sources: {
          aerial: {
            type: "raster",
            tiles: [`${tileBase}/aerial/{z}/{x}/{y}`],
            tileSize: 256,
            minzoom: 2,
            maxzoom: 11,
          },
          "dem-terrain": {
            type: "raster-dem",
            tiles: [`${tileBase}/dem/{z}/{x}/{y}`],
            tileSize: 256,
            encoding: "terrarium",
            minzoom: 1,
            maxzoom: 11,
          },
        },
        layers: [
          {
            id: "background",
            type: "background",
            paint: { "background-color": "#888888" },
          },
          { id: "aerial-layer", type: "raster", source: "aerial", paint: {} },
        ],
      },
      center: [130.4, 33.6],
      zoom: 10,
      minZoom: 3,
      pitch: 60,
      bearing: -20,
    });

    map.on("load", () => {
      map!.setTerrain({ source: "dem-terrain", exaggeration: 1.5 });

      overlay = new MapboxOverlay({ layers: [] });
      map!.addControl(overlay as unknown as maplibregl.IControl);
      mapLoaded = true;
      applyDeckLayers();
    });

    return () => {
      overlay?.finalize();
      if (map) map.remove();
    };
  });
</script>

<div bind:this={mapContainer} class="map-wrap"></div>

<style>
  .map-wrap {
    width: 100%;
    height: 100%;
  }
</style>
