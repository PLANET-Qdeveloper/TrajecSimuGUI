<script lang="ts">
  import { onMount } from 'svelte';
  import maplibregl from 'maplibre-gl';
  import 'maplibre-gl/dist/maplibre-gl.css';

  let mapContainer: HTMLDivElement;
  let map: maplibregl.Map;

  onMount(() => {
    map = new maplibregl.Map({
      container: mapContainer,
      // ローカルのタイルを読み込むためのカスタムスタイル定義
      style: {
        version: 8,
        sources: {
          'local-aerial': {
            type: 'raster',
            // {z}/{x}/{y} はMapLibreが自動的に計算して展開します
            // Svelteのpublicフォルダ(ルート)にある tiles フォルダを参照
            tiles: ['/tiles/{z}/{x}/{y}.jpg'], 
            tileSize: 256,
            minzoom: 7,  // ZOOM_LEVELS の最小値に合わせる
            maxzoom: 10  // ZOOM_LEVELS の最大値に合わせる
          }
        },
        layers: [
          {
            id: 'background',
            type: 'background',
            paint: {
              'background-color': '#888888'
            }
          },
          {
            id: 'local-aerial-layer',
            type: 'raster',
            source: 'local-aerial',
            paint: {}
          }
        ]
      },
      center: [130.4, 33.6], // 福岡市周辺
      zoom: 10,
      minZoom: 7,
      pitch: 45 // 3D風に傾ける
    });

    map.on('load', () => {
      // 線の描画（サンプル）
      map.addSource('route', {
        'type': 'geojson',
        'data': {
          'type': 'Feature',
          'properties': {},
          'geometry': {
            'type': 'LineString',
            'coordinates': [
              [130.38, 33.59],
              [130.42, 33.59]
            ]
          }
        }
      });
      map.addLayer({
        'id': 'route',
        'type': 'line',
        'source': 'route',
        'layout': { 'line-join': 'round', 'line-cap': 'round' },
        'paint': { 'line-color': '#00ff00', 'line-width': 5 } // 緑色の線
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