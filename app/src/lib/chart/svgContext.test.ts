import { describe, expect, it } from 'vitest';
import { SvgContext, escapeXml, parseFont } from './svgContext';

const measure = (text: string) => text.length * 6;

describe('SvgContext', () => {
  it('turns_filled_and_stroked_paths_into_path_elements_with_colours_dashes_and_alpha', () => {
    const c = new SvgContext(100, 50, measure);
    c.fillStyle = '#abc';
    c.beginPath();
    c.rect(1, 2, 30, 40);
    c.moveTo(0, 0);
    c.lineTo(5, 5.126);
    c.closePath();
    c.fill();
    c.strokeStyle = 'rgb(1, 2, 3)';
    c.lineWidth = 1.5;
    c.setLineDash([4, 2]);
    c.globalAlpha = 0.25;
    c.beginPath();
    c.moveTo(0, 0);
    c.lineTo(10, 0);
    c.stroke();
    const svg = c.toSvg('#fff', { w: 100, h: 20 });
    expect(svg).toContain('<path d="M1 2h30v40h-30ZM0 0L5 5.13Z" fill="#abc"/>');
    expect(svg).toContain('<path d="M0 0L10 0" fill="none" stroke="rgb(1, 2, 3)" stroke-width="1.5" stroke-dasharray="4 2" opacity="0.25"/>');
  });

  it('applies_the_current_transform_to_each_element_and_skips_the_identity', () => {
    const c = new SvgContext(10, 10, measure);
    c.fillRect(0, 0, 1, 1);
    c.setTransform(2, 0, 0, 2, -4, -6);
    c.fillRect(0, 0, 1, 1);
    const svg = c.toSvg('#fff', { w: 10, h: 10 });
    expect(svg).toContain('<path d="M0 0h1v1h-1Z" fill="#000"/>');
    expect(svg).toContain('<path d="M0 0h1v1h-1Z" fill="#000" transform="matrix(2 0 0 2 -4 -6)"/>');
  });

  it('writes_text_with_alignment_font_and_escapes_it', () => {
    const c = new SvgContext(10, 10, measure);
    c.font = '600 12px "Segoe UI", sans-serif';
    c.textAlign = 'right';
    c.textBaseline = 'middle';
    c.fillStyle = '#111';
    c.fillText('A & <B> "q"', 3, 4);
    c.fillText('', 0, 0);
    const svg = c.toSvg('#fff', { w: 10, h: 10 });
    expect(svg).toContain('font-family="&quot;Segoe UI&quot;, sans-serif"');
    expect(svg).toContain('font-size="12" font-weight="600" text-anchor="end" dominant-baseline="central"');
    expect(svg).toContain('>A &amp; &lt;B&gt; &quot;q&quot;</text>');
    expect(svg.match(/<text /g)).toHaveLength(1);
  });

  it('measures_text_with_the_font_that_is_set_and_draws_circles_and_arcs', () => {
    const seen: string[] = [];
    const c = new SvgContext(10, 10, (text, font) => {
      seen.push(font);
      return text.length * 7;
    });
    c.font = '13px serif';
    expect(c.measureText('abcd').width).toBe(28);
    expect(seen).toEqual(['13px serif']);
    c.beginPath();
    c.arc(10, 10, 4, 0, Math.PI * 2);
    c.fill();
    expect(c.toSvg('#fff', { w: 10, h: 10 })).toContain('M6 10a4 4 0 1 0 8 0a4 4 0 1 0 -8 0Z');
  });

  it('makes_a_well_formed_document_with_the_chart_clipped_in_a_nested_svg', () => {
    const c = new SvgContext(120, 90, measure);
    const svg = c.toSvg('#fbfbf8', { w: 120, h: 60 }, '<text>cap</text>\n');
    expect(svg.startsWith('<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" width="120" height="90"')).toBe(true);
    expect(svg).toContain('<rect width="120" height="90" fill="#fbfbf8"/>');
    expect(svg).toContain('<svg x="0" y="0" width="120" height="60" viewBox="0 0 120 60" overflow="hidden">');
    expect(svg.indexOf('<text>cap</text>')).toBeGreaterThan(svg.indexOf('</svg>'));
    const doc = new DOMParser().parseFromString(svg, 'image/svg+xml');
    expect(doc.getElementsByTagName('parsererror')).toHaveLength(0);
  });
});

describe('helpers', () => {
  it('escapes_xml_and_drops_characters_xml_cannot_hold', () => {
    expect(escapeXml('a<b>&"c\u0000d\u0007e')).toBe('a&lt;b&gt;&amp;&quot;cde');
  });

  it('splits_a_canvas_font_string', () => {
    expect(parseFont('italic bold 14.5px Georgia, serif')).toEqual({ style: 'italic', weight: 'bold', size: 14.5, family: 'Georgia, serif' });
    expect(parseFont('10px sans-serif')).toEqual({ style: 'normal', weight: 'normal', size: 10, family: 'sans-serif' });
    expect(parseFont('nonsense').size).toBe(12);
  });
});
