<script lang="ts">
  import { onMount } from 'svelte';
  import maplibregl from 'maplibre-gl';
  import 'maplibre-gl/dist/maplibre-gl.css';

  let mapContainer: HTMLDivElement;
  let map: maplibregl.Map;

  onMount(() => {
    map = new maplibregl.Map({
      container: mapContainer,
      style: {
        version: 8,
        sources: {
          'aerial': {
            type: 'raster',
            tiles: ['tile://localhost/aerial/{z}/{x}/{y}'],
            tileSize: 256,
            minzoom: 2,
            maxzoom: 11
          },
          'dem-terrain': {
            type: 'raster-dem',
            tiles: ['tile://localhost/dem/{z}/{x}/{y}'],
            tileSize: 256,
            encoding: 'terrarium',
            minzoom: 1,
            maxzoom: 11
          }
        },
        layers: [
          {
            id: 'background',
            type: 'background',
            paint: { 'background-color': '#888888' }
          },
          {
            id: 'aerial-layer',
            type: 'raster',
            source: 'aerial',
            paint: {}
          }
        ]
      },
      center: [130.4, 33.6], // 福岡市周辺
      zoom: 10,
      minZoom: 7,
      pitch: 60,
      bearing: -20
    });

    map.on('load', () => {
      // 3Dテレイン有効化
      map.setTerrain({ source: 'dem-terrain', exaggeration: 1.5 });

      // 線の描画（サンプル）
      map.addSource('route', {
        type: 'geojson',
        data: {
          type: 'Feature',
          properties: {},
          geometry: {
            type: 'LineString',
            coordinates: [
              [130.38, 33.59],
              [130.42, 33.59]
            ]
          }
        }
      });
      map.addLayer({
        id: 'route',
        type: 'line',
        source: 'route',
        layout: { 'line-join': 'round', 'line-cap': 'round' },
        paint: { 'line-color': '#00ff00', 'line-width': 5 }
      });
    });

    return () => {
      if (map) map.remove();
    };
  });
</script>

<div bind:this={mapContainer} class="map-wrap"></div>

<style>
  .map-wrap {
    width: 100%;
    height: 100vh;
  }
</style>
