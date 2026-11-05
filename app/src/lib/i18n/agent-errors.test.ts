import { afterEach, describe, expect, it } from 'vitest';
import accessSource from '../../../src-tauri/src/agent_access.rs?raw';
import settingsSource from '../../../src-tauri/src/settings_file.rs?raw';
import { agentErrorText } from './backend';
import { setLanguage } from '.';

afterEach(() => setLanguage('en'));

const SAMPLES: Record<string, string> = {
  port: '9100',
  e: 'os error 10013',
  names: 'Reader, Second',
  id: 'not-an-id',
  name: 'Reader',
  what: 'settings',
};

/** Every sentence in a Rust source: a string literal that starts with a capital and has a space. */
function messages(source: string): string[] {
  const code = source
    .split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');
  return [...code.matchAll(/"((?:[^"\\]|\\.)*)"/g)]
    .map((m) => m[1].replace(/\\"/g, '"'))
    .filter((text) => /^[A-Z]/.test(text) && text.includes(' '));
}

function filled(template: string): string {
  return template.replace(/\{(\w+)\}/g, (_, name: string) => {
    expect(SAMPLES, `a sample for {${name}} in: ${template}`).toHaveProperty(name);
    return SAMPLES[name];
  });
}

const found = [...messages(accessSource), ...messages(settingsSource)];

describe('agentErrorText', () => {
  it('reads_the_messages_from_the_rust_sources', () => {
    expect(found).toContain('Port {port} is already used by another program. Choose another port.');
    expect(found).toContain('Could not save {what}: {e}');
    expect(found.length).toBeGreaterThanOrEqual(15);
  });

  it.each(found)('has_both_languages_for: %s', (template) => {
    const text = filled(template);
    expect(agentErrorText(text), 'the English entry is the backend text').toBe(text);
    setLanguage('uk');
    const uk = agentErrorText(text);
    expect(uk, 'no Ukrainian entry').not.toBe(text);
    expect(uk).toMatch(/[а-яіїєґ]/i);
    expect(uk).not.toContain('{');
    for (const name of template.matchAll(/\{(\w+)\}/g)) {
      if (name[1] !== 'what') expect(uk).toContain(SAMPLES[name[1]]);
    }
    expect(uk).not.toMatch(/дерев/i);
  });

  it('ukrainian_port_taken_is_as_agreed', () => {
    setLanguage('uk');
    expect(agentErrorText('Port 9100 is already used by another program. Choose another port.')).toBe(
      'Порт 9100 вже використовує інша програма. Виберіть інший порт.',
    );
  });

  it('other_text_is_shown_as_it_came', () => {
    setLanguage('uk');
    expect(agentErrorText('Something nobody planned for.')).toBe('Something nobody planned for.');
  });
});
