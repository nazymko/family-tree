import { mount } from 'svelte';
import '../styles/fonts.css';
import '../styles/tokens.css';
import '../styles/base.css';
import MapsHarness from './MapsHarness.svelte';

mount(MapsHarness, { target: document.getElementById('app')! });
