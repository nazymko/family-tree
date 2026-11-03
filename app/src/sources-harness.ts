// Dev-only page (sources.html): shows the sources, media, export and settings views on MockApi.
import { mount } from 'svelte';
import './styles/fonts.css';
import './styles/tokens.css';
import './styles/base.css';
import Harness from './lib/testing/SourcesHarness.svelte';

mount(Harness, { target: document.getElementById('app')! });
