import { mount } from 'svelte';

async function boot() {
  const target = document.getElementById('app')!;
  if (import.meta.env.VITE_GT_MODE === 'spike') {
    const { default: Spike } = await import('./spike/Spike.svelte');
    mount(Spike, { target });
  } else {
    const { default: App } = await import('./App.svelte');
    mount(App, { target });
  }
}

void boot();
