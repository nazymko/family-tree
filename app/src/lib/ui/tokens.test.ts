import { describe, expect, it } from 'vitest';
import tokens from '../../styles/tokens.css?raw';
import { contrast } from './contrast';

function declarations(body: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of body.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) out[m[1]] = m[2].trim();
  return out;
}

/** Return the body of the first innermost `{ ... }` block whose selector text matches. */
function block(css: string, selector: RegExp): string {
  const re = new RegExp(selector.source + '\\s*\\{([^{}]*)\\}', 's');
  const m = css.match(re);
  if (!m) throw new Error(`no block for ${selector}`);
  return m[1];
}

const light = declarations(block(tokens, /:root(?![:[])/));
const dark = declarations(block(tokens, /:root\[data-theme="dark"\]/));
const darkMedia = declarations(block(tokens, /:root:not\(\[data-theme="light"\]\)/));

const colourTokens = [
  '--paper-0', '--paper-1', '--paper-2', '--rule', '--ink-0', '--ink-1', '--ink-2',
  '--ribbon', '--ribbon-wash', '--on-ribbon', '--verdigris', '--foxing', '--foxing-wash',
  '--danger', '--focus',
];

describe('tokens', () => {
  for (const [name, t] of [['light', light], ['dark', dark]] as const) {
    describe(name, () => {
      it('defines_every_colour_token', () => {
        for (const k of colourTokens) expect(t[k], k).toMatch(/^#[0-9A-Fa-f]{6}$/);
      });
      it('ink_0_on_paper_0_is_7', () => {
        expect(contrast(t['--ink-0'], t['--paper-0'])).toBeGreaterThanOrEqual(7);
      });
      it('ink_1_and_2_on_paper_1', () => {
        expect(contrast(t['--ink-1'], t['--paper-1'])).toBeGreaterThanOrEqual(4.5);
        expect(contrast(t['--ink-2'], t['--paper-1'])).toBeGreaterThanOrEqual(4.5);
      });
      it('ribbon_on_paper_1', () => {
        expect(contrast(t['--ribbon'], t['--paper-1'])).toBeGreaterThanOrEqual(4.5);
      });
      it('foxing_on_wash', () => {
        expect(contrast(t['--foxing'], t['--foxing-wash'])).toBeGreaterThanOrEqual(4.5);
      });
      it('focus_and_rule', () => {
        expect(contrast(t['--focus'], t['--paper-1'])).toBeGreaterThanOrEqual(3);
        expect(contrast(t['--rule'], t['--paper-1'])).toBeGreaterThanOrEqual(1.3);
      });
      it('status_colours_readable', () => {
        expect(contrast(t['--danger'], t['--paper-1'])).toBeGreaterThanOrEqual(4.5);
        expect(contrast(t['--verdigris'], t['--paper-1'])).toBeGreaterThanOrEqual(4.5);
      });
    });
  }

  it('on_ribbon_on_ribbon_light', () => {
    expect(contrast(light['--on-ribbon'], light['--ribbon'])).toBeGreaterThanOrEqual(4.5);
  });

  it('media_query_dark_matches_attribute_dark', () => {
    expect(darkMedia).toEqual(dark);
  });

  it('contrast_helper_known_values', () => {
    expect(contrast('#000000', '#FFFFFF')).toBeCloseTo(21, 1);
    expect(contrast('#777777', '#777777')).toBeCloseTo(1, 5);
  });
});
