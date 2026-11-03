import { beforeEach, describe, expect, it } from 'vitest';
import { nav } from './nav.svelte';

describe('nav', () => {
  beforeEach(() => nav.reset());

  it('starts_on_welcome', () => {
    expect(nav.route).toEqual({ name: 'welcome' });
    expect(nav.canBack).toBe(false);
    expect(nav.canForward).toBe(false);
  });

  it('back_forward_restores_routes', () => {
    nav.go({ name: 'family' });
    nav.go({ name: 'people' });
    nav.go({ name: 'sources' });
    nav.back();
    expect(nav.route).toEqual({ name: 'people' });
    nav.back();
    expect(nav.route).toEqual({ name: 'family' });
    nav.forward();
    expect(nav.route).toEqual({ name: 'people' });
    nav.go({ name: 'media' });
    expect(nav.canForward).toBe(false);
    nav.back();
    nav.back();
    nav.back();
    expect(nav.route).toEqual({ name: 'welcome' });
    nav.back();
    expect(nav.route).toEqual({ name: 'welcome' });
  });

  it('go_same_route_does_not_stack', () => {
    nav.go({ name: 'people' });
    nav.go({ name: 'people' });
    nav.back();
    expect(nav.route).toEqual({ name: 'welcome' });
  });

  it('focus_person_follows_person_routes', () => {
    expect(nav.focusPerson).toBeNull();
    nav.go({ name: 'person', id: 'a' });
    expect(nav.focusPerson).toBe('a');
    nav.go({ name: 'people' });
    expect(nav.focusPerson).toBe('a');
    nav.go({ name: 'person', id: 'b' });
    expect(nav.focusPerson).toBe('b');
    nav.back();
    nav.back();
    expect(nav.route).toEqual({ name: 'person', id: 'a' });
    expect(nav.focusPerson).toBe('a');
  });

  it('focus_sets_person_without_changing_route', () => {
    nav.go({ name: 'family' });
    nav.focus('z');
    expect(nav.focusPerson).toBe('z');
    expect(nav.route).toEqual({ name: 'family' });
  });
});
