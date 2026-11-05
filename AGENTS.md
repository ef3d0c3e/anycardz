<!-- lific:begin -->
## Issue tracking: Lific

This project tracks its work in Lific, as project `AC` (https://lific.pundalik.org/AC/overview).

> The tracker database was lost and repopulated from git history on 2026-10-24, so issue numbers cited in older commit messages use the previous numbering; each recreated issue notes its original id in its description.

Use the Lific MCP tools for issues, pages, and plans when they are connected. Use the `lific` CLI for work that needs this machine. The MCP server can run on another host, so it cannot see your files, working directory, or git history. The CLI also has every data command, for when MCP is not connected. Add `--json` for machine-readable output:

```bash
lific issue list --project AC --json      # browse issues
lific issue get AC-1                       # one issue with relations
lific issue update AC-1 --status done      # close an issue
lific search "auth flow" --project AC      # full-text search
```

Rules:
- When you finish an issue, set its status to `done`.
- File each issue in the right module, and keep it to one change.
- Use a plan for work that spans sessions.
- An issue assigned to a person (`for:` in lists) is theirs. Take only unassigned issues.
<!-- lific:end -->

## Development

- `make dev` runs Postgres (Docker, host port 5433), the API (:8080) and the SvelteKit dev server (:5173). `make test` and `make check` before finishing a change.
- SvelteKit 3: config lives in `vite.config.ts`, `$lib` is now the `#lib` subpath import and needs file extensions (`#lib/api.ts`), hook types come from `@sveltejs/kit/hooks`, `goto` has no `noScroll`, use `refreshAll` instead of `invalidateAll`.
- diesel-async 0.9: `conn.transaction(async move |conn| { ... })`, no `scope_boxed`. Locking (`for_update`) does not work on boxed queries.
- Shared client state (i18n, theme, session, catalog, toasts) goes through Svelte context, never module-level `$state`: on the server one module serves every visitor.
- UI strings live in `frontend/src/lib/i18n/{en,fr}.ts`. Add every new key to both.

## Tests

Every bug fix comes with a test that fails without the fix.

- Backend rules and endpoints: `backend/src/**` unit tests and `backend/tests/api.rs` (real Postgres, run races with `join_all`).
- Frontend logic: keep it out of components in plain modules (e.g. `src/lib/opening/reels.ts`) and test it with Vitest next to it (`*.test.ts`, fake timers for anything timed).
- Flows: Playwright specs in `frontend/e2e/`, run on a phone and a desktop project. Create players with `newPlayer()` (registers through the API, can buy boosters), navigate with `visit()` (waits for hydration: clicks on server-rendered markup do nothing until then). Prefer roles and visible text in locators.
