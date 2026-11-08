# Contributing to Family Tree

Thank you for helping. Bug reports, ideas, translations, documentation and code are all welcome.

## The one firm rule: no real people's data

Family trees hold names, dates and places of living people. **Never put real people's data in an issue, a screenshot, a
test, a fixture or a commit**, your own family included. Use invented names (`Ivan Petrovych Koval`, `Anna Aaberg`) or
the generated test tree (`python scripts/make-synthetic-ged.py out.ged --persons 500`). If a bug only shows with your
own tree, describe its shape ("a family with three children and no parents entered") instead of attaching the file.

## Reporting a bug or asking for a feature

Use the [issue forms](https://github.com/nazymko/gene-tree/issues/new/choose). For a bug, say what you did, what you
expected and what happened, plus the version (from the file name you downloaded) and your operating system. Security problems go privately,
see [SECURITY.md](SECURITY.md).

## Making a change

1. **Fork** the repository and clone your fork.
2. Create a branch from `develop`: `git switch -c fix/short-description develop`.
3. Set up: [Rust](https://rustup.rs), Node.js 20+ and, on Linux, the WebKitGTK packages in [docs/release.md](docs/release.md).
   Then `cd app && npm ci`.
4. Run the app: `npm run tauri dev` (inside `app/`).
5. Make the change with tests (below), then open a **pull request against `develop`**.

### Branches

| Branch | What it is |
|---|---|
| `develop` | Where changes land. Pull requests target it. |
| `main` | What the nightly build and the monthly releases are made from. Updated from `develop` once its tests pass. |
| `vX.Y.Z` tags | Releases. Made by the release workflow, never by hand. |

### Tests

```bash
bash scripts/test.sh --quick    # only what your change affects, seconds for a small change
bash scripts/test.sh --full     # what CI runs: fmt, clippy, all Rust tests, vitest, svelte-check, eslint
bash scripts/test.sh --e2e      # the real app driven through WebDriver (Windows and Linux)
```

Every fix gets a test that fails without it. [docs/testing.md](docs/testing.md) explains the tiers.

### Code style

- Rust: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` must be clean.
- Frontend: `npm run check` and `npm run lint` (inside `app/`) must be clean.
- The TypeScript types in `app/src/lib/api/generated` come from Rust: change the Rust type and run
  `UPDATE_BINDINGS=1 cargo test -p gt-session --test export_bindings`; never edit them by hand.
- Interface text lives in `app/src/lib/i18n/en.ts` and `uk.ts`; add every new string to both. In Ukrainian a family
  tree is «родовід».
- Write plain, short sentences in code comments and messages, as the existing code does.

### Commit messages

`type(scope): what changed`, for example `fix(layout): brothers and sisters of a family with no parents are drawn`.
Types: `feat` (users see something new), `fix` (users see something work), `perf`, `test`, `docs`, `refactor`,
`chore`. The release notes are made from `feat`, `fix` and `perf` commits, so write those for a user to read.

## License

By contributing you agree that your work is dual-licensed under the [MIT License](LICENSE-MIT) and the
[Apache License 2.0](LICENSE-APACHE), as the rest of the project.
