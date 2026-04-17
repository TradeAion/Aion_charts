# Framework Integration Guide

AxiusCharts is framework-agnostic. The stable pattern is: initialize WASM once, create one chart per mounted container, dispose on unmount, and feed data through `Float64Array` / `BigUint64Array` without any precision adapters.

## React

```tsx
import { useEffect, useRef } from 'react';
import init, { AxiusCharts } from 'axiuscharts-wasm';

const wasmReady = init();

export function Chart() {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<AxiusCharts | null>(null);

  useEffect(() => {
    let disposed = false;

    (async () => {
      await wasmReady;
      if (disposed || !containerRef.current) return;

      const chart = await AxiusCharts.create_chart(containerRef.current, {
        renderer: 'auto',
        autoRender: true,
        theme: 'dark',
      });

      if (disposed) {
        chart.dispose();
        return;
      }

      chartRef.current = chart;
    })();

    return () => {
      disposed = true;
      chartRef.current?.dispose();
      chartRef.current = null;
    };
  }, []);

  return <div ref={containerRef} style={{ width: '100%', height: 420 }} />;
}
```

## Vue 3

```vue
<template>
  <div ref="container" style="width: 100%; height: 420px" />
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue';
import init, { AxiusCharts } from 'axiuscharts-wasm';

const container = ref<HTMLDivElement | null>(null);
let chart: AxiusCharts | null = null;

onMounted(async () => {
  await init();
  if (!container.value) return;
  chart = await AxiusCharts.create_chart(container.value, { renderer: 'auto' });
});

onBeforeUnmount(() => {
  chart?.dispose();
  chart = null;
});
</script>
```

## Svelte

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import init, { AxiusCharts } from 'axiuscharts-wasm';

  let container: HTMLDivElement;
  let chart: AxiusCharts | null = null;

  onMount(async () => {
    await init();
    chart = await AxiusCharts.create_chart(container, { renderer: 'auto' });
  });

  onDestroy(() => {
    chart?.dispose();
  });
</script>

<div bind:this={container} style="width: 100%; height: 420px" />
```

## Bundlers

- Vite: exclude `axiuscharts-wasm` from dependency prebundling if needed
- webpack 5: enable `experiments.asyncWebAssembly`
- Next.js: wire async WebAssembly through the webpack config

## Events

AxiusCharts uses camelCase event payload fields:

```ts
chart.on('crosshairMove', (e) => {
  console.log(e.price, e.timestamp, e.barIndex);
});

chart.on('visibleRangeChange', ({ startBar, endBar }) => {
  console.log(startBar, endBar);
});
```

Kinetic glide updates also emit `visibleRangeChange`, so synchronization code does not need a separate "gesture ended" fallback path.
