import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PagedListHost from './testing/PagedListHost.svelte';

function mount(total = 50000, extra: Record<string, unknown> = {}) {
  const loadPage = vi.fn(async (offset: number, limit: number) =>
    Array.from({ length: Math.min(limit, total - offset) }, (_, i) => `Row ${offset + i}`),
  );
  const onSelect = vi.fn();
  const onActivate = vi.fn();
  const result = render(PagedListHost, { total, loadPage, onSelect, onActivate, ...extra });
  return { loadPage, onSelect, onActivate, ...result };
}

const box = () => screen.getByRole('listbox', { name: 'People list' });

describe('PagedList', () => {
  it('renders_window_not_all', async () => {
    mount();
    await waitFor(() => expect(screen.getByText('Row 0')).toBeInTheDocument());
    expect(screen.getAllByRole('option').length).toBeLessThan(60);
  });

  it('loads_pages_on_scroll', async () => {
    const { loadPage } = mount();
    await waitFor(() => expect(loadPage).toHaveBeenCalledWith(0, 200));
    box().scrollTop = 40 * 1000;
    await fireEvent.scroll(box());
    await waitFor(() => expect(loadPage.mock.calls.some(([o, l]) => (o === 800 || o === 1000) && l === 200)).toBe(true));
    expect(loadPage.mock.calls.filter(([o]) => o === 0)).toHaveLength(1);
    await waitFor(() => expect(screen.getByText('Row 1000')).toBeInTheDocument());
  });

  it('lru_evicts', async () => {
    const { loadPage } = mount();
    await waitFor(() => expect(loadPage).toHaveBeenCalledWith(0, 200));
    for (let p = 1; p <= 21; p++) {
      box().scrollTop = p * 200 * 40;
      await fireEvent.scroll(box());
      await waitFor(() => expect(loadPage.mock.calls.some(([o]) => o === p * 200)).toBe(true));
    }
    box().scrollTop = 0;
    await fireEvent.scroll(box());
    await waitFor(() => expect(loadPage.mock.calls.filter(([o]) => o === 0).length).toBe(2));
  });

  it('placeholder_while_loading', async () => {
    mount(50000, { loadPage: () => new Promise(() => {}) });
    const first = screen.getAllByRole('option')[0];
    expect(first).toHaveAttribute('data-loading', 'true');
    expect(first).toHaveClass('loading');
    expect(first).toHaveTextContent('loading');
  });

  it('keyboard_navigation', async () => {
    const user = userEvent.setup();
    const { onSelect, onActivate } = mount(50000, { selectedIndex: 0 });
    box().focus();
    await user.keyboard('{End}');
    expect(onSelect).toHaveBeenLastCalledWith(49999);
    await user.keyboard('{Home}');
    expect(onSelect).toHaveBeenLastCalledWith(0);
    await user.keyboard('{ArrowDown}');
    expect(onSelect).toHaveBeenLastCalledWith(1);
    await user.keyboard('{Enter}');
    expect(onActivate).toHaveBeenCalledWith(0);
  });

  it('aria_counts', async () => {
    mount();
    expect(box()).toHaveAttribute('aria-rowcount', '50000');
    expect(screen.getAllByRole('option')[0]).toHaveAttribute('aria-rowindex', '1');
  });

  it('initial_page_avoids_duplicate_request', async () => {
    const { loadPage } = mount(1000, { initialPage: { offset: 0, items: Array.from({ length: 200 }, (_, i) => `Row ${i}`) } });
    await new Promise((r) => setTimeout(r, 20));
    expect(loadPage).not.toHaveBeenCalled();
    expect(screen.getByText('Row 3')).toBeInTheDocument();
  });
});
