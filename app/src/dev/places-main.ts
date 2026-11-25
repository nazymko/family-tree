import { mount } from 'svelte';
import '../styles/fonts.css';
import '../styles/tokens.css';
import '../styles/base.css';
import PlacesHarness from './PlacesHarness.svelte';

mount(PlacesHarness, { target: document.getElementById('app')! });
