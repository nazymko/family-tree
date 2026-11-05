import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { agentChanges, REFRESH_MS } from './agentChanges.svelte';
import { project } from './project.svelte';

describe('a burst of agent events', () => {
  let stop: () => void = () => {};
  let raw: MockApi;
  let platform: MockPlatform;
  const fire = (id = 'p1') => platform.simulateAgentEvent([id]);

  beforeEach(() => {
    vi.useFakeTimers();
    raw = new MockApi(fixtureTree(5));
    platform = new MockPlatform();
    agentChanges.reset();
    stop = agentChanges.start(raw, platform);
  });

  afterEach(() => {
    stop();
    agentChanges.reset();
    vi.useRealTimers();
  });

  it('refreshes_at_once_then_once_more_at_the_end', () => {
    const rev = project.rev;
    for (let i = 0; i < 20; i++) fire();
    expect(project.rev).toBe(rev + 1);
    vi.advanceTimersByTime(REFRESH_MS - 1);
    expect(project.rev).toBe(rev + 1);
    vi.advanceTimersByTime(1);
    expect(project.rev).toBe(rev + 2);
    vi.advanceTimersByTime(REFRESH_MS * 10);
    expect(project.rev).toBe(rev + 2);
  });

  it('a_lone_event_refreshes_once', () => {
    const rev = project.rev;
    fire();
    expect(project.rev).toBe(rev + 1);
    vi.advanceTimersByTime(REFRESH_MS * 10);
    expect(project.rev).toBe(rev + 1);
  });

  it('a_long_steady_stream_refreshes_at_most_once_per_interval', () => {
    const rev = project.rev;
    const step = Math.ceil(REFRESH_MS / 5);
    const rounds = 50;
    for (let i = 0; i < rounds; i++) {
      fire();
      vi.advanceTimersByTime(step);
    }
    vi.advanceTimersByTime(REFRESH_MS * 2);
    const refreshes = project.rev - rev;
    expect(refreshes).toBeLessThanOrEqual(Math.ceil((rounds * step) / REFRESH_MS) + 1);
    expect(refreshes).toBeGreaterThan(1);
  });

  it('a_quiet_gap_lets_the_next_event_through_at_once', () => {
    const rev = project.rev;
    fire();
    vi.advanceTimersByTime(REFRESH_MS * 3);
    fire();
    expect(project.rev).toBe(rev + 2);
  });

  it('what_was_touched_is_known_at_once_even_when_the_refresh_waits', () => {
    fire('p1');
    fire('p2');
    expect(agentChanges.seq).toBe(2);
    expect(agentChanges.touchedSince(['p2'], 1)).toBe(true);
    expect(agentChanges.lastTouched).toEqual(['p2']);
  });

  it('coalesces_the_badge_and_undo_reads_too', () => {
    const badges = vi.spyOn(raw, 'unreviewedPeople');
    const buttons = vi.spyOn(raw, 'undoState');
    for (let i = 0; i < 20; i++) fire();
    vi.advanceTimersByTime(REFRESH_MS * 3);
    expect(badges).toHaveBeenCalledTimes(2);
    expect(buttons).toHaveBeenCalledTimes(2);
  });

  it('stopping_drops_the_refresh_still_waiting', () => {
    const rev = project.rev;
    fire();
    fire();
    stop();
    vi.advanceTimersByTime(REFRESH_MS * 3);
    expect(project.rev).toBe(rev + 1);
  });

  it('reset_drops_the_refresh_still_waiting', () => {
    const rev = project.rev;
    fire();
    fire();
    agentChanges.reset();
    vi.advanceTimersByTime(REFRESH_MS * 3);
    expect(project.rev).toBe(rev + 1);
  });
});
