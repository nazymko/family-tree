import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';
import globals from 'globals';

const a11y = Object.fromEntries(
  Object.keys(svelte.rules)
    .filter((r) => r.startsWith('a11y-'))
    .map((r) => [`svelte/${r}`, 'error']),
);

export default tseslint.config(
  { ignores: ['dist/**', 'node_modules/**', 'src-tauri/**', 'playwright-report/**', 'test-results/**'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    languageOptions: { globals: { ...globals.browser, ...globals.node } },
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts'],
    languageOptions: { parserOptions: { parser: tseslint.parser } },
  },
  { files: ['**/*.svelte'], rules: { ...a11y } },
);
