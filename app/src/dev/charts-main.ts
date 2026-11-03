import { mount } from 'svelte';
import '../styles/fonts.css';
import '../styles/tokens.css';
import '../styles/base.css';
import ChartsHarness from './ChartsHarness.svelte';

mount(ChartsHarness, { target: document.getElementById('app')! });
