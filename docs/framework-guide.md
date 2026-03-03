# Framework Integration Guide

RayCore is framework-agnostic — it works with any JavaScript framework. The key pattern is: **initialize WASM once, mount/unmount chart instances per component lifecycle**.

---

## React

```tsx
import { useEffect, useRef } from 'react';
import init, { RayCore } from 'raycore-wasm';

// Initialize WASM once at module level
const wasmReady = init();

interface ChartProps {
  theme?: 'dark' | 'light';
  symbol?: string;
  interval?: string;
}

export function Chart({ theme = 'dark', symbol = 'BTCUSD', interval = '1m' }: ChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<RayCore | null>(null);

  // Mount
  useEffect(() => {
    let disposed = false;

    (async () => {
      await wasmReady;
      if (disposed || !containerRef.current) return;

      const chart = await RayCore.create_chart(containerRef.current, {
        renderer: 'webgpu',
        autoRender: true,
        theme,
        symbol,
        interval,
      });

      if (disposed) { chart.dispose(); return; }
      chartRef.current = chart;
    })();

    return () => {
      disposed = true;
      chartRef.current?.dispose();
      chartRef.current = null;
    };
  }, []); // mount once

  // React to prop changes
  useEffect(() => {
    chartRef.current?.apply_options({ theme });
  }, [theme]);

  return <div ref={containerRef} style={{ width: '100%', height: '400px' }} />;
}
```

### Common React Pitfalls

- **StrictMode double-mount**: React 18 StrictMode mounts/unmounts/remounts in development. The `disposed` flag prevents stale chart references.
- **HMR cleanup**: Ensure `dispose()` runs on hot reload. The pattern above handles this via the useEffect cleanup.
- **Container sizing**: The `div` must have explicit height. Flexbox `flex: 1` works if the parent has a defined height.

---

## Vue 3 (Composition API)

```vue
<template>
  <div ref="container" style="width: 100%; height: 400px" />
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue';
import init, { RayCore } from 'raycore-wasm';

const props = defineProps<{ theme?: 'dark' | 'light' }>();

const container = ref<HTMLDivElement>();
let chart: RayCore | null = null;

onMounted(async () => {
  await init();
  if (!container.value) return;

  chart = await RayCore.create_chart(container.value, {
    renderer: 'webgpu',
    autoRender: true,
    theme: props.theme ?? 'dark',
  });
});

onBeforeUnmount(() => {
  chart?.dispose();
  chart = null;
});

watch(() => props.theme, (t) => {
  chart?.apply_options({ theme: t });
});
</script>
```

---

## Svelte

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import init, { RayCore } from 'raycore-wasm';

  export let theme: 'dark' | 'light' = 'dark';

  let container: HTMLDivElement;
  let chart: RayCore | null = null;

  onMount(async () => {
    await init();
    chart = await RayCore.create_chart(container, {
      renderer: 'webgpu',
      autoRender: true,
      theme,
    });
  });

  onDestroy(() => {
    chart?.dispose();
  });

  $: if (chart) chart.apply_options({ theme });
</script>

<div bind:this={container} style="width: 100%; height: 400px;" />
```

---

## Vanilla JavaScript

```html
<div id="chart" style="width: 100%; height: 400px;"></div>
<script type="module">
  import init, { RayCore } from './pkg/raycore_wasm.js';

  await init();

  const chart = await RayCore.create_chart('chart', {
    renderer: 'webgpu',
    autoRender: true,
    theme: 'dark',
  });

  // Load data, attach events, etc.
  chart.demo_mode();
</script>
```

---

## Bundler Configuration

### Vite

Vite handles WASM files natively. Just ensure the `.wasm` file is included in the build output:

```ts
// vite.config.ts
export default defineConfig({
  optimizeDeps: {
    exclude: ['raycore-wasm'],  // don't pre-bundle WASM
  },
});
```

### webpack 5

```js
// webpack.config.js
module.exports = {
  experiments: {
    asyncWebAssembly: true,
  },
};
```

### Next.js

```js
// next.config.js
module.exports = {
  webpack: (config) => {
    config.experiments = { ...config.experiments, asyncWebAssembly: true };
    return config;
  },
};
```

---

## Events

RayCore provides a typed event system:

```ts
chart.on('crosshairMove', (e) => {
  console.log(e.price, e.timestamp, e.bar_index);
});

chart.on('click', (e) => { /* ... */ });
chart.on('visibleRangeChange', (e) => { /* ... */ });
chart.on('drawingCreated', (e) => { /* ... */ });
chart.on('drawingSelected', (e) => { /* ... */ });

// Unsubscribe
const handler = (e) => { /* ... */ };
chart.on('crosshairMove', handler);
chart.off('crosshairMove', handler);

// One-time listener
chart.once('resize', (e) => { /* ... */ });
```
