import { mount } from 'svelte';
import '../styles/fonts.css';
import '../styles/tokens.css';
import '../styles/base.css';
import Spike from './Spike.svelte';

mount(Spike, { target: document.getElementById('app')! });
