<script lang="ts">
  import { onMount } from "svelte";
  import maplibregl from "maplibre-gl";
  import "maplibre-gl/dist/maplibre-gl.css";
  import { getTileBaseUrl } from "$lib/utils/tileBaseUrl";
  import { kml as kmlToGeoJson } from "@tmcw/togeojson";
  import { MapboxOverlay } from "@deck.gl/mapbox";
  import { ScatterplotLayer } from "@deck.gl/layers";
  import type { Layer } from "@deck.gl/core";

  interface Props {
    latitude: number[];
    longitude: number[];
    altitude: number[];
    value: number[];
    overlayKmlString: string | null;
    showImportedKmlOverlay: boolean;
    visible?: boolean;
    mapLoaded?: boolean;
  }

  let {
    latitude = [],
    longitude = [],
    altitude = [],
    value = [],
    overlayKmlString = null,
    showImportedKmlOverlay = false,
    visible = true,
    mapLoaded = $bindable(false),
  }: Props = $props();

  let mapContainer: HTMLDivElement;
  let map: maplibregl.Map | undefined;
  let overlay: MapboxOverlay | undefined;

  function turboColor(t: number): [number, number, number, number] {
    t = Math.max(0, Math.min(1, t));
    const t2 = t * t;
    const t3 = t2 * t;
    const t4 = t3 * t;
    const t5 = t4 * t;
    const r =
      0.13572138 +
      4.6153926 * t -
      42.66032258 * t2 +
      132.13108234 * t3 -
      152.94239396 * t4 +
      59.28637943 * t5;
    const g =
      0.09140261 +
      2.19418839 * t +
      4.84296658 * t2 -
      14.18503333 * t3 +
      4.27729857 * t4 +
      2.82956604 * t5;
    const b =
      0.1066733 +
      12.64194608 * t -
      60.58204836 * t2 +
      110.36276771 * t3 -
      89.90310912 * t4 +
      27.34824973 * t5;
    return [
      Math.round(255 * Math.max(0, Math.min(1, r))),
      Math.round(255 * Math.max(0, Math.min(1, g))),
      Math.round(255 * Math.max(0, Math.min(1, b))),
      255,
    ];
  }

  function parseKml(kmlStr: string) {
    const parser = new DOMParser();
    const doc = parser.parseFromString(kmlStr, "text/xml");
    return kmlToGeoJson(doc);
  }

  function applyDeckLayers() {
    if (!overlay) return;

    const minVal = Math.min(...value);
    const maxVal = Math.max(...value);
    const range = maxVal - minVal || 1;

    const data = latitude.map((lat, i) => ({
      position: [longitude[i], lat, altitude[i]] as [number, number, number],
      t: (value[i] - minVal) / range,
    }));

    overlay.setProps({
      layers: [
        new ScatterplotLayer({
          id: "chart-points",
          data,
          getPosition: (d: { position: [number, number, number] }) =>
            d.position,
          getRadius: 1,
          radiusUnits: "pixels",
          getFillColor: (d: { t: number }) => turboColor(d.t),
          stroked: false,
          billboard: true,
          pickable: false,
          parameters: { depthTest: false },
        }),
      ] as Layer[],
    });

    if (map && data.length > 0) {
      const lngs = data.map((d) => d.position[0]);
      const lats = data.map((d) => d.position[1]);
      const bounds = new maplibregl.LngLatBounds(
        [Math.min(...lngs), Math.min(...lats)],
        [Math.max(...lngs), Math.max(...lats)],
      );
      map.fitBounds(bounds, { padding: 60, maxZoom: 14 });
    }
  }

  function updateOverlay(kmlStr: string | null) {
    if (!map || !mapLoaded) return;
    const source = map.getSource("kml-overlay") as
      | maplibregl.GeoJSONSource
      | undefined;
    if (!source) return;
    if (!kmlStr) {
      source.setData({ type: "FeatureCollection", features: [] });
      return;
    }
    if (showImportedKmlOverlay) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      source.setData(parseKml(kmlStr) as any);
    } else {
      source.setData({ type: "FeatureCollection", features: [] });
    }
  }

  $effect(() => {
    void [latitude, longitude, altitude, value];
    applyDeckLayers();
  });
  $effect(() => {
    updateOverlay(overlayKmlString);
  });
  $effect(() => {
    if (visible && map) {
      requestAnimationFrame(() => map?.resize());
    }
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
      minZoom: 7,
      pitch: 60,
      bearing: -20,
    });

    map.on("load", () => {
      map!.setTerrain({ source: "dem-terrain", exaggeration: 1.5 });

      map!.addSource("kml-overlay", {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
      });
      map!.addLayer({
        id: "kml-overlay-lines",
        type: "line",
        source: "kml-overlay",
        filter: ["==", ["geometry-type"], "LineString"],
        layout: { "line-join": "round", "line-cap": "round" },
        paint: { "line-color": "#0ea5e9", "line-width": 2 },
      });
      map!.addLayer({
        id: "kml-overlay-points",
        type: "circle",
        source: "kml-overlay",
        filter: ["==", ["geometry-type"], "Point"],
        paint: {
          "circle-radius": 4,
          "circle-color": "#0ea5e9",
          "circle-stroke-width": 1.5,
          "circle-stroke-color": "#ffffff",
        },
      });

      overlay = new MapboxOverlay({ layers: [] });
      map!.addControl(overlay as unknown as maplibregl.IControl);
      applyDeckLayers();

      mapLoaded = true;
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
