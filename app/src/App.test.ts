import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import App from './App.svelte';

describe('App', () => {
  it('renders_root_landmark', () => {
    render(App);
    expect(screen.getByRole('main')).toBeInTheDocument();
    expect(document.documentElement.lang).toBe('en');
  });
});
