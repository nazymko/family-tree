---
name: translate-missing
description: Use when the user asks to translate missing UI strings into the other languages (the 19 partial locales: zh-Hans, hi, es, ar, fr, bn, pt, id, ur, de, ja, mr, vi, te, ha, tr, sw, tl, ta), or to refresh stale translations after English changed.
---

# Translate the missing UI strings

Policy and file layout: `docs/i18n.md`. English (`en.ts`) and Ukrainian (`uk.ts`) are complete and maintained by feature work; the 19
other languages are partial files in `app/src/lib/i18n/locales/` that you fill. Missing keys fall back to English, so a partial job is
always safe to ship. Language of your own status lines and commit messages: English or Ukrainian only. Never Russian; never add a Russian locale.

## Steps

1. **Measure.** `node scripts/i18n-missing.mjs` shows translated / stale / missing per language. Pick the languages (all, or those named).
2. **Plan batches.** Per language, take a slice with `node scripts/i18n-missing.mjs --locale <code> --json --section <prefix> --limit 150 > batch.json`
   (put `batch.json` in the scratchpad, not the repo). Sections are the top-level groups of `en.ts` (`nav`, `shell`, `settings`, `person`, ...);
   do the visible ones first (`nav`, `shell`, `common`, `settings`, `ui`, `people`, `person`, `family`, `chart`). One subagent per language is fine;
   tell each: English or Ukrainian status lines only, synthetic data only, run `i18n-apply` in the foreground.
3. **Translate.** For each item in `items`, write the translation of `en` (use `uk` as a second reference for meaning, never copy it for a
   non-Slavic language). Output a flat JSON `{ "<key>": "<text>" | { "<category>": "<text>" } }`. Rules:
   - Keep every `{placeholder}` exactly as in English (`{n}`, `{dir}`, `{name}`...). Do not translate or rename them.
   - Plural entries (`en` is an object with `one`/`other`): give exactly the CLDR categories of the language. `pluralCategories` in the batch tells
     which: `ja`, `zh-Hans`, `id`, `vi`: `other`; `hi`, `bn`, `ur`, `de`, `mr`, `te`, `ha`, `tr`, `sw`, `tl`, `ta`: `one`, `other`; `es`, `fr`, `pt`:
     `one`, `many`, `other` (`many` is for millions: "{n} de personas"); `ar`: `zero`, `one`, `two`, `few`, `many`, `other`. `other` keeps `{n}`;
     `one`/`two`/`zero` may drop it ("شخصان").
   - Genealogy terminology: a "tree" is a family tree (genealogy), a "pedigree" is an ancestor chart, "descendants" a descendant chart, "sources" are
     citation sources (not "origins"), a "citation" is a cited reference, "home person" is the person the view opens on. Use the normal genealogy
     word of the language (es árbol genealógico, fr arbre généalogique, de Stammbaum, ja 家系図, zh 家谱, hi वंशवृक्ष, ar شجرة العائلة / شجرة النسب).
     Keep it consistent across batches: read the language's file (`locales/<code>.ts`) for the words already chosen.
   - Keep the register of the English: short button labels stay short; sentences stay sentences. Keep punctuation like `…` and quotes natural for the language.
   - Do not translate: product name "Family Tree", file formats (GEDCOM), keyboard key names, `{...}` placeholders, numbers.
   - Real people's names never appear; use synthetic examples only.
4. **Apply.** `node scripts/i18n-apply.mjs <code> translated.json`. Read the summary: fix every `rejected` line and apply again. The tool rewrites the
   locale file, its `.meta.json` hashes and `locales/coverage.json`.
5. **Stale keys.** `i18n-missing` lists them with `status: "stale"` and the old text in `current`: retranslate from the new English and apply again.
6. **Verify.** `node --test "scripts/*.test.mjs"`, then in `app/`: `node node_modules/vitest/vitest.mjs run src/lib/i18n`, then `bash scripts/test.sh --quick`
   (foreground). The i18n tests fail if a locale has a key English lacks or a different placeholder set.
7. **Look at it.** Take screenshots of a few screens (sidebar, a person page, Settings) in one RTL language (`ar` or `ur`) and one CJK language (`ja` or
   `zh-Hans`) with the window-spec harness (`GT_E2E_SHOTS`, see `app/e2e-tauri/specs/language-rtl.e2e.ts` for a template): check mirrored layout,
   clipped or overflowing text, missing glyphs (boxes), and English left in the middle of a translated screen.
8. **Commit** the locale files, `.meta.json` and `coverage.json` together. Do not hand-edit them afterwards; redo the step with the tools.

A language reaching 50 % is picked automatically for users whose system language is it; below that it is only offered in the Settings list.
