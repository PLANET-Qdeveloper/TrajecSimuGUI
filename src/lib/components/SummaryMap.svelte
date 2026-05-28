<script lang="ts">
  import { onMount } from "svelte";
  import maplibregl from "maplibre-gl";
  import "maplibre-gl/dist/maplibre-gl.css";
  import { getTileBaseUrl } from "$lib/utils/tileBaseUrl";
  import { kml as kmlToGeoJson } from "@tmcw/togeojson";
  import { MapboxOverlay } from "@deck.gl/mapbox";
  import { PathLayer, ScatterplotLayer, TextLayer } from "@deck.gl/layers";
  import type { Layer } from "@deck.gl/core";

  interface Props {
    kmlString: string | null;
    overlayKmlString: string | null;
    landingAreaKmlString: string | null;
    showTrajectoryMarker: boolean;
    showBallisticCourse: boolean;
    showParachuteCourse: boolean;
    showBallisticLandingRange: boolean;
    showParachuteLandingRange: boolean;
    showImportedKmlOverlay: boolean;
    visible?: boolean;
    mapLoaded?: boolean;
  }

  let {
    kmlString = null,
    overlayKmlString = null,
    landingAreaKmlString = null,
    showTrajectoryMarker = false,
    showBallisticCourse = false,
    showParachuteCourse = false,
    showBallisticLandingRange = false,
    showParachuteLandingRange = false,
    showImportedKmlOverlay = false,
    visible = true,
    mapLoaded = $bindable(false),
  }: Props = $props();

  let mapContainer: HTMLDivElement;
  let map: maplibregl.Map | undefined;
  let overlay: MapboxOverlay | undefined;

  let trajectoryLayers: Layer[] = [];
  let landingAreaLayers: Layer[] = [];

  function applyDeckLayers() {
    overlay?.setProps({ layers: [...trajectoryLayers, ...landingAreaLayers] });
  }

  function parseKml(kmlStr: string) {
    const parser = new DOMParser();
    const doc = parser.parseFromString(kmlStr, "text/xml");
    return kmlToGeoJson(doc);
  }

  function fitBoundGeoJson(geojson: ReturnType<typeof parseKml>) {
    if (!map) return;
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

  function updateTrajectory(kmlStr: string | null) {
    if (!map || !mapLoaded || !overlay) return;

    if (!kmlStr) {
      trajectoryLayers = [];
      applyDeckLayers();
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

    const points_ballistic = geojson.features
      .filter((f) => f.geometry?.type === "Point")
      .filter((f) => f.properties?.styleUrl === "#event_ballistic")
      .map((f) => ({
        position: (f.geometry as { coordinates: number[] }).coordinates as [
          number,
          number,
          number,
        ],
        text: f.properties?.name,
      }));

    const points_parachute = geojson.features
      .filter((f) => f.geometry?.type === "Point")
      .filter((f) => f.properties?.styleUrl === "#event_parachute")
      .map((f) => ({
        position: (f.geometry as { coordinates: number[] }).coordinates as [
          number,
          number,
          number,
        ],
        text: f.properties?.name,
      }));

    trajectoryLayers = [
      new PathLayer({
        id: "trajectories_ballistic",
        data: ballistic_paths,
        getPath: (d: { path: [number, number, number][] }) => d.path,
        getColor: [213, 94, 0, 255],
        getWidth: 3,
        visible: showBallisticCourse,
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
        visible: showParachuteCourse,
        widthUnits: "pixels",
        billboard: true,
        jointRounded: true,
        pickable: false,
      }),
      new ScatterplotLayer({
        id: "traj_events_ballistic",
        data: points_ballistic,
        getPosition: (d: { position: [number, number, number] }) => d.position,
        getRadius: 5,
        radiusUnits: "pixels",
        getFillColor: [213, 94, 0, 255],
        getLineColor: [50, 50, 50, 255],
        visible: showTrajectoryMarker && showBallisticCourse,
        stroked: true,
        lineWidthUnits: "pixels",
        billboard: true,
        getLineWidth: 2,
        pickable: false,
      }),
      new TextLayer({
        id: "traj_event_labels_ballistic",
        data: points_ballistic,
        getPosition: (d: { position: [number, number, number] }) => d.position,
        getText: (d: { text?: string }) => d.text ?? "",
        getSize: 14,
        sizeUnits: "pixels",
        getColor: [255, 255, 255, 255],
        getHaloColor: [0, 0, 0, 200],
        visible: showTrajectoryMarker && showBallisticCourse,
        haloWidth: 1,
        billboard: true,
        getAlignmentBaseline: "bottom",
        pickable: false,
      }),
      new ScatterplotLayer({
        id: "traj_events_parachute",
        data: points_parachute,
        getPosition: (d: { position: [number, number, number] }) => d.position,
        getRadius: 5,
        radiusUnits: "pixels",
        getFillColor: [204, 121, 167, 255],
        getLineColor: [50, 50, 50, 255],
        visible: showTrajectoryMarker && showParachuteCourse,
        stroked: true,
        lineWidthUnits: "pixels",
        billboard: true,
        getLineWidth: 2,
        pickable: false,
      }),
      new TextLayer({
        id: "traj_event_labels_parachute",
        data: points_parachute,
        getPosition: (d: { position: [number, number, number] }) => d.position,
        getText: (d: { text?: string }) => d.text ?? "",
        getSize: 14,
        sizeUnits: "pixels",
        getColor: [255, 255, 255, 255],
        getHaloColor: [0, 0, 0, 200],
        visible: showTrajectoryMarker && showParachuteCourse,
        haloWidth: 1,
        billboard: true,
        getAlignmentBaseline: "bottom",
        pickable: false,
      }),
    ];
    applyDeckLayers();

    fitBoundGeoJson(geojson);
  }

  function updateLandingArea(kmlStr: string | null) {
    if (!map || !mapLoaded || !overlay) return;

    if (!kmlStr) {
      landingAreaLayers = [];
      applyDeckLayers();
      return;
    }
    const geojson = parseKml(kmlStr);
    const ballistic_paths = geojson.features
      .filter((f) => f.geometry?.type === "Polygon")
      .filter((f) => f.properties?.styleUrl === "#ballistic_hull")
      .map((f) => ({
        name: (f.properties?.name ?? "") as string,
        path: (f.geometry as { coordinates: number[][][] }).coordinates[0] as [
          number,
          number,
          number,
        ][],
      }));

    const parachute_paths = geojson.features
      .filter((f) => f.geometry?.type === "Polygon")
      .filter((f) => f.properties?.styleUrl === "#parachute_hull")
      .map((f) => ({
        name: (f.properties?.name ?? "") as string,
        path: (f.geometry as { coordinates: number[][][] }).coordinates[0] as [
          number,
          number,
          number,
        ][],
      }));

    const points_ballistic = geojson.features
      .filter((f) => f.geometry?.type === "Point")
      .filter((f) => f.properties?.styleUrl === "#ballistic_pt")
      .map((f) => ({
        position: (f.geometry as { coordinates: number[] }).coordinates as [
          number,
          number,
          number,
        ],
        text: f.properties?.name,
      }));

    const points_parachute = geojson.features
      .filter((f) => f.geometry?.type === "Point")
      .filter((f) => f.properties?.styleUrl === "#parachute_pt")
      .map((f) => ({
        position: (f.geometry as { coordinates: number[] }).coordinates as [
          number,
          number,
          number,
        ],
        text: f.properties?.name,
      }));

    landingAreaLayers = [
      new PathLayer({
        id: "landing_area_ballistic",
        data: ballistic_paths,
        getPath: (d: { path: [number, number, number][] }) => d.path,
        getColor: [213, 94, 0, 255],
        getWidth: 1,
        widthUnits: "pixels",
        visible: showBallisticLandingRange,
        billboard: true,
        jointRounded: true,
        pickable: false,
      }),
      new PathLayer({
        id: "landing_area_parachute",
        data: parachute_paths,
        getPath: (d: { path: [number, number, number][] }) => d.path,
        getColor: [204, 121, 167, 255],
        getWidth: 1,
        widthUnits: "pixels",
        visible: showParachuteLandingRange,
        billboard: true,
        jointRounded: true,
        pickable: false,
      }),
      new ScatterplotLayer({
        id: "landing_area_events_ballistic",
        data: points_ballistic,
        getPosition: (d: { position: [number, number, number] }) => d.position,
        getRadius: 2,
        radiusUnits: "pixels",
        getFillColor: [213, 94, 0, 255],
        getLineColor: [50, 50, 50, 255],
        stroked: true,
        visible: showTrajectoryMarker && showBallisticLandingRange,
        lineWidthUnits: "pixels",
        billboard: true,
        getLineWidth: 1,
        pickable: false,
      }),
      new ScatterplotLayer({
        id: "landing_area_events_parachute",
        data: points_parachute,
        getPosition: (d: { position: [number, number, number] }) => d.position,
        getRadius: 2,
        radiusUnits: "pixels",
        getFillColor: [204, 121, 167, 255],
        getLineColor: [50, 50, 50, 255],
        stroked: true,
        visible: showTrajectoryMarker && showParachuteLandingRange,
        lineWidthUnits: "pixels",
        billboard: true,
        getLineWidth: 1,
        pickable: false,
      }),
    ];
    applyDeckLayers();
    fitBoundGeoJson(geojson);
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
