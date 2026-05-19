<script lang="ts">
  import { onMount } from "svelte";
  import maplibregl from "maplibre-gl";
  import "maplibre-gl/dist/maplibre-gl.css";
  import { kml as kmlToGeoJson } from "@tmcw/togeojson";
  import { MapboxOverlay } from "@deck.gl/mapbox";
  import { PathLayer, ScatterplotLayer, TextLayer } from "@deck.gl/layers";

  let {
    kmlString = null,
    overlayKmlString = null,
    landingAreaKmlString = null,
    visible = true,
    mapLoaded = $bindable(false),
  }: {
    kmlString: string | null;
    overlayKmlString: string | null;
    landingAreaKmlString: string | null;
    visible?: boolean;
    mapLoaded?: boolean;
  } = $props();

  let mapContainer: HTMLDivElement;
  let map: maplibregl.Map | undefined;
  let overlay: MapboxOverlay | undefined;

  function parseKml(kmlStr: string) {
    const parser = new DOMParser();
    const doc = parser.parseFromString(kmlStr, "text/xml");
    return kmlToGeoJson(doc);
  }

  function updateTrajectory(kmlStr: string | null) {
    if (!map || !mapLoaded || !overlay) return;

    if (!kmlStr) {
      overlay.setProps({ layers: [] });
      return;
    }

    const geojson = parseKml(kmlStr);
    const ballistic_paths = geojson.features
      .filter((f) => f.geometry?.type === "LineString")
      .filter((f) => f.properties?.name === "Ballistic phase")
      .map((f) => ({
        name: (f.properties?.name ?? "") as string,
        path: (f.geometry as { coordinates: number[][] }).coordinates as [
          number,
          number,
          number,
        ][],
      }));
    console.log(geojson);
    const parachute_paths = geojson.features
      .filter((f) => f.geometry?.type === "LineString")
      .filter((f) => f.properties?.name === "Parachute descent")
      .map((f) => ({
        name: (f.properties?.name ?? "") as string,
        path: (f.geometry as { coordinates: number[][] }).coordinates as [
          number,
          number,
          number,
        ][],
      }));

    const points = geojson.features
      .filter((f) => f.geometry?.type === "Point")
      .map((f) => ({
        position: (f.geometry as { coordinates: number[] }).coordinates as [
          number,
          number,
          number,
        ],
        text: f.properties?.name,
      }));

    overlay.setProps({
      layers: [
        new PathLayer({
          id: "trajectories_ballistic",
          data: ballistic_paths,
          getPath: (d: { path: [number, number, number][] }) => d.path,
          getColor: [213, 94, 0, 255],
          getWidth: 3,
          widthUnits: "pixels",
          billboard: true,
          jointRounded: true,
          pickable: false,
        }),
        new PathLayer({
          id: "trajectories_parachute",
          data: parachute_paths,
          getPath: (d: { path: [number, number, number][] }) => d.path,
          getColor: [204, 121, 167, 255],
          getWidth: 3,
          widthUnits: "pixels",
          billboard: true,
          jointRounded: true,
          pickable: false,
        }),
        new ScatterplotLayer({
          id: "events",
          data: points,
          getPosition: (d: { position: [number, number, number] }) =>
            d.position,
          getRadius: 5,
          radiusUnits: "pixels",
          getFillColor: [255, 255, 255, 255],
          getLineColor: [50, 50, 50, 255],
          stroked: true,
          lineWidthUnits: "pixels",
          billboard: true,
          getLineWidth: 2,
          pickable: false,
        }),
        new TextLayer({
          id: "event-labels",
          data: points,
          getPosition: (d: { position: [number, number, number] }) =>
            d.position,
          getText: (d: { text?: string }) => d.text ?? "",
          getSize: 14,
          sizeUnits: "pixels",
          getColor: [255, 255, 255, 255],
          getHaloColor: [0, 0, 0, 200],
          haloWidth: 1,
          billboard: true,
          getAlignmentBaseline: "bottom",
          pickable: false,
        }),
      ],
    });

    const coords: [number, number][] = [];
    for (const f of geojson.features) {
      const geom = f.geometry;
      if (!geom) continue;
      if (geom.type === "LineString") {
        for (const c of (geom as { coordinates: number[][] }).coordinates) {
          if (c && c.length >= 2 && !isNaN(c[0]) && !isNaN(c[1])) {
            coords.push([c[0], c[1]]);
          }
        }
      } else if (geom.type === "Point") {
        const c = (geom as { coordinates: number[] }).coordinates;
        if (c && c.length >= 2 && !isNaN(c[0]) && !isNaN(c[1])) {
          coords.push([c[0], c[1]]);
        }
      }
    }
    if (coords.length > 0) {
      const bounds = coords.reduce(
        (b, c) => b.extend(c),
        new maplibregl.LngLatBounds(coords[0], coords[0]),
      );
      map.fitBounds(bounds, { padding: 60, maxZoom: 14 });
    }
  }

  function updateLandingArea(kmlStr: string | null) {
    if (!map || !mapLoaded) return;
    const source = map.getSource("kml-landing-area") as
      | maplibregl.GeoJSONSource
      | undefined;
    if (!source) return;
    if (!kmlStr) {
      source.setData({ type: "FeatureCollection", features: [] });
      return;
    }
    const geojson = parseKml(kmlStr);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    source.setData(geojson as any);

    const coords: [number, number][] = [];
    for (const f of geojson.features) {
      const geom = f.geometry;
      if (!geom) continue;
      if (geom.type === "Point") {
        const c = (geom as { coordinates: number[] }).coordinates;
        coords.push([c[0], c[1]]);
      } else if (geom.type === "Polygon") {
        for (const ring of (geom as { coordinates: number[][][] })
          .coordinates) {
          for (const c of ring) coords.push([c[0], c[1]]);
        }
      }
    }
    if (coords.length > 0) {
      const bounds = coords.reduce(
        (b, c) => b.extend(c),
        new maplibregl.LngLatBounds(coords[0], coords[0]),
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
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    source.setData(parseKml(kmlStr) as any);
  }

  $effect(() => {
    updateTrajectory(kmlString);
  });
  $effect(() => {
    updateOverlay(overlayKmlString);
  });
  $effect(() => {
    updateLandingArea(landingAreaKmlString);
  });
  $effect(() => {
    if (visible && map) {
      requestAnimationFrame(() => map?.resize());
    }
  });

  onMount(() => {
    map = new maplibregl.Map({
      container: mapContainer,
      style: {
        version: 8,
        sources: {
          aerial: {
            type: "raster",
            tiles: ["tile://localhost/aerial/{z}/{x}/{y}"],
            tileSize: 256,
            minzoom: 2,
            maxzoom: 11,
          },
          "dem-terrain": {
            type: "raster-dem",
            tiles: ["tile://localhost/dem/{z}/{x}/{y}"],
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

      map!.addSource("kml-landing-area", {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
      });
      map!.addLayer({
        id: "kml-landing-area-fill",
        type: "fill",
        source: "kml-landing-area",
        filter: ["==", ["geometry-type"], "Polygon"],
        paint: { "fill-color": "#005ed5", "fill-opacity": 0.2 },
      });
      map!.addLayer({
        id: "kml-landing-area-outline",
        type: "line",
        source: "kml-landing-area",
        filter: ["==", ["geometry-type"], "Polygon"],
        paint: { "line-color": "#005ed5", "line-width": 2 },
      });
      map!.addLayer({
        id: "kml-landing-area-points",
        type: "circle",
        source: "kml-landing-area",
        filter: ["==", ["geometry-type"], "Point"],
        paint: {
          "circle-radius": 4,
          "circle-color": "#005ed5",
          "circle-stroke-width": 1.5,
          "circle-stroke-color": "#ffffff",
        },
      });

      overlay = new MapboxOverlay({ layers: [] });
      map!.addControl(overlay as unknown as maplibregl.IControl);

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
