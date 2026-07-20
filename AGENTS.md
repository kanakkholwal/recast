# AGENTS.md

Engineering guidelines for **Recast** — an offline-first desktop screen recorder/editor
(Tauri v2 + Rust + Svelte 5) plus a SvelteKit cloud web app. This file is the contract every
contributor follows, **human or AI agent**. It is intentionally tool-agnostic (works with any
agent that reads `AGENTS.md`). Read it before writing code; treat the RULES as hard
requirements, not suggestions.

> Per-area `AGENTS.md` files may refine (never contradict) this one.

---

## 1. Stack & layout

**Monorepo:** pnpm `10.x` + Turborepo. `"type": "module"` everywhere. TypeScript `5.9`, strict.

```
apps/
  desktop/        Tauri v2 app. Frontend: SvelteKit (adapter-static, SPA, SSR off) + Svelte 5.
                  Backend: Rust in src-tauri/ (recorder, editor, export, cloud sync).
  web/            SvelteKit 2 on Vercel. Server: Hono 4 + Drizzle ORM + Postgres + Zod 4 +
                  better-auth. Storage: S3 / R2 / Azure via files-sdk. Payments: Polar.
packages/
  ui/             Shared Svelte components (shadcn-svelte + bits-ui).
  design/         Design tokens / Tailwind 4 theme.
  player/         Shared video player.
  analytics/      PostHog swap-abstraction.
```

Shared frontend: **Svelte 5.56**, **Vite 7**, **Tailwind 4**, **Tabler icons** (primary) +
**Phosphor duotone** accents (AI-surface only — see §4), **shadcn-svelte**, **bits-ui**.
Desktop tests: **Vitest 3** (pure logic) + `cargo test`.

### Commands (run from repo root unless noted)

```bash
pnpm dev                      # all apps via turbo
pnpm dev:desktop | dev:web    # one app
pnpm build | check | lint | fmt:check   # turbo fan-out — must be green before "done"

# Desktop (apps/desktop/)
pnpm dev                      # tauri dev
pnpm check                    # svelte-check (frontend types)
pnpm test                     # vitest + cargo test
pnpm lint                     # cargo clippy -D warnings
pnpm fmt:check                # cargo fmt --check

# Web (apps/web/)
pnpm check                    # svelte-check
pnpm db:generate | db:migrate # drizzle-kit (NEVER db:push to prod)
```

---

## 2. Golden rules (non-negotiable, all languages)

1. **Match the surrounding code.** Comment density, naming, idioms, file structure. New code
   should be indistinguishable from what's already there. Don't introduce a new pattern when an
   established one exists.
   - **Comments earn their place: explain *why*, not *what*.** No narration that restates the code,
     no decorative section banners, no generic AI-boilerplate phrasing ("This function is
     responsible for…", "Note that…"). A comment that an experienced reader would skip shouldn't
     exist. Keep the ones that encode a *why* or a bug history (see rule 2).
2. **Surgical changes, not rewrites.** This is a mature codebase. Fix the thing asked; don't
   "modernize" untouched code. Preserve invariant-bearing comments — if you move code, move its
   comment. Many comments encode hard-won platform bug history; deleting them re-opens bugs.
3. **Validate every boundary.** All external input (IPC payloads, HTTP body/query/params, env,
   third-party responses, file paths) is untrusted until parsed/validated. Parse, don't guess.
4. **Never swallow errors silently.** Best-effort cleanup may ignore failures *with a comment
   saying so*; a user-facing operation that fails must be logged and surfaced.
5. **No secrets in client code or git.** Server-only env stays server-only. No hard-coded keys.
6. **Type the contract end-to-end.** No `any`, no lying casts. `unknown` at boundaries, then
   narrow. Serialized shapes (IPC, HTTP) are contracts — change them in lockstep on both sides.
7. **Heavy work never blocks the UI/event loop.** (Rust: `spawn_blocking`; Node: no sync I/O on
   the request path.) See the per-area rules.
8. **Leave the gates green.** `fmt`, `lint`/`clippy`, `check` (svelte-check), and tests must pass
   before a change is "done." A red build is never a stopping point.
9. **Small, reviewable commits**, one concern each. Conventional Commit messages
   (`feat(editor): …`, `fix(macos): …`). Work on a branch; open a PR; **let the maintainer own
   merges to `main`** — don't push to or force-push shared branches.
10. **When unsure about a product/behavioural decision, ask** — don't guess at scope. When the
    answer is in the code or a sensible default exists, act and say what you assumed.
11. **Separate concerns; don't duplicate.** Pure logic, reactive state, and markup/lifecycle each
    live in their own place — pure logic in `.ts`/`.logic.ts`, shared reactive state in a
    `.svelte.ts` rune module, markup + lifecycle in `.svelte`; on the backend, business logic in a
    service module, not inside the command/handler (see §3, §4). One source of truth for any rule —
    extract a shared helper instead of copy-pasting, and keep state flow one-directional and
    explicit (derive, don't sync).
12. **NLE performance budget violations are merge-blocking.** The `@recast/media` package's
    performance budgets (see `packages/media/REQUIREMENTS.md §3`) are tested by
    `packages/media/test/perf/budgets.test.ts`. Any PR that causes a regression on an *enforced*
    row fails the build. Note that only the two memory-cap rows are enforced in Node today; the
    latency, bundle-size and audio-drift rows need a browser harness that does not exist yet and
    are listed as unenforced in that file. Do not add a placeholder assertion that passes
    vacuously to make a row look covered — that is exactly how five runtime bugs survived three
    PRs (see `packages/media/MIGRATION-LOG.md` PR-G).

---

## 3. Rust / Tauri backend

The desktop backend is performance- and concurrency-critical (live capture, FFmpeg, threads).

### State & concurrency
- **Pick the cheapest correct primitive per field — don't default everything to `Mutex`:**
  read-mostly config → `parking_lot::RwLock`; a single flag → `Arc<AtomicBool>`; a counter →
  `AtomicU64`; set-once-read-many → `OnceLock`; genuinely-mutable shared map → `parking_lot::Mutex`.
- **`parking_lot` only** for mutexes/rwlocks (no poisoning → a panicked holder can't abort the
  app). Do not introduce `std::sync::Mutex`/`RwLock`.
- **Never hold a lock across `.await`, blocking I/O, or a process spawn.** Idiom: lock → copy the
  minimal data into a local → drop the guard → do the slow work.
- **`Arc::clone` is a refcount bump** (cheap, correct for sharing into threads). A deep `.clone()`
  of a big struct is fine *once per operation* but a **bug in a hot loop** (per-frame, per-IPC-tick).
- Choose atomic `Ordering` by what memory it publishes: pure flag/counter guarding no other data →
  `Relaxed`; a store handing off data → `Release`/`Acquire`. Don't cargo-cult `SeqCst`.

### Threads & processes (leak-proofing)
- **Every owned thread/child process gets an RAII `Drop`** that signals its stop-flag and joins —
  so a panic or early `?` can never orphan an FFmpeg child or a spinning capture thread. Don't
  rely on cleanup code at the bottom of a long function.
- **Long-lived child with piped stderr/stdout MUST be drained continuously** on a side thread, or
  the ~64 KB OS pipe fills and the child deadlocks mid-recording.
- **Every `ffmpeg`/`ffprobe` spawn calls `configure_silent_command`** — on Windows a console flash
  steals focus and reads as "the whole window froze."
- **Don't present partial output as valid** — a hard-killed encoder / truncated file must be
  detected (probe, not a size heuristic) and rejected or salvaged explicitly.

### Tauri command layer
- **The macOS rule:** sync Tauri commands run on the main thread and freeze the macOS WKWebView
  (Windows WebView2 masks it, so it looks fine on Windows and hangs on Mac). Any command doing
  heavy CPU / blocking I/O / process spawn MUST be `async fn` + `tauri::async_runtime::spawn_blocking`.
  Trivial in-memory getters may stay sync. There is a regression test asserting this — keep it green.
- **Commands are thin IPC adapters:** deserialize → call a domain service → map error → return.
  No FFmpeg arg-building, project-zipping, or business logic inline in a `#[tauri::command]`.
  Domain logic lives in `AppHandle`-free, unit-testable service modules (this is also what makes
  the app automation/MCP-ready).
- **Boundary errors are typed**, not stringly-typed: a `thiserror` enum that derives `Serialize`
  (`{ kind, message }`) so the frontend can branch on *kind*. Internally use `anyhow::Result` with
  rich `.context()`; convert with the full chain (`{e:#}`), never a lossy `.to_string()`. Never
  smuggle control flow (e.g. cancellation) through an error *string*.
- **All IPC structs are `#[serde(rename_all = "camelCase")]`** — the field names are a contract
  with the Svelte caller. Renaming one means updating every `invoke()` site + TS type in the same
  change.

### Persistence & platform
- **Atomic file writes:** write to `*.tmp` in the same dir → fsync → `rename` over the target.
  Never truncate-then-write a file the app reads on next launch (config, project).
- **`#[cfg(target_os)]` stays at the `platform/mod.rs` boundary** behind a uniform trait. Don't
  leak `cfg` blocks into domain/command code. All target trees must compile
  (windows/macos/linux_x11/linux_wayland/fallback).
- **Plugin registration:** JS-injecting plugins (os, dialog, sharekit) go on the `Builder` before
  any window — not inside `setup()` (too late; the bundle already loaded without the init script).

### Verify (from `apps/desktop/src-tauri/`)
`cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo test` · plus
`cargo check` the host target and reason through the `cfg` branches for the others.

---

## 4. Frontend — Svelte 5 + SvelteKit 2

Current as of **Svelte 5.36+ / SvelteKit 2.12+**. Runes era — no Svelte 4 idioms in new code.

### Runes & reactivity
- **`$state` only for genuinely reactive values**; plain `let` otherwise (proxies have cost).
- **Derive, don't sync — `$derived`/`$derived.by`, almost never `$effect`.** `$effect` is an
  escape hatch for analytics/logging/DOM/3rd-party sync only; **never use it to write one
  `$state` from another.**
  ```ts
  // ❌ let doubled=$state(); $effect(()=>doubled=count*2)
  let doubled = $derived(count * 2);            // ✅
  ```
- **`$state.raw`** for large immutable data (no proxy; reassign, don't mutate).
- **`$state.snapshot(x)`** before handing reactive state to non-Svelte APIs (`structuredClone`,
  serializers, **Tauri `invoke`** — proxies don't serialize).
- **Reactive collections** from `svelte/reactivity` (`SvelteMap`/`SvelteSet`), not native `Map`/`Set`.

### Components
- **Snippets + `{@render}`, never `<slot>`** (slots are legacy). Default content is `children`.
- **Callback props, never `createEventDispatcher`** (deprecated). Events are function props
  (`onsave`). DOM events are attributes (`onclick`, not `on:click`; modifiers are gone).
- **Type props with an `interface Props`**; snippets as `Snippet<[Args]>`. Two-way binding only
  when the child opts in via `$bindable()` — prefer callback props.

### State management
- **Local `$state`** by default. Shared logic → a **`.svelte.ts` rune module** (the store
  replacement): export an object/class and mutate it, don't `export let x = $state()` and reassign
  across files.
- **Context API (`setContext`/`getContext`) for per-tree shared state**, not globals.
- **NEVER put request/user state in a module-level global on the SvelteKit *server*** — it leaks
  across concurrent users (the #1 SSR footgun). Module-scope rune state is fine in the *client /
  Tauri* app only.

### SvelteKit data flow
- **`+page.server.ts` for DB/secrets/cookies; `+page.ts` (universal) for public data.** `load`
  functions are pure — return data, never write to stores/globals. Recompute view-data with
  `$derived(data.…)`.
- **Read page/nav via `$app/state` (runes-based), not `$app/stores`** (deprecated, removed in Kit 3).
- **Avoid waterfalls** (start independent fetches before `await parent()`); **stream slow data**
  by returning un-awaited promises.
- **Mutations: form actions + `use:enhance`** as the default (works without JS). `+server.ts` for
  JSON APIs/webhooks.

### Server/client boundary
- **`$lib/server/**` and `*.server.ts` are compile-enforced server-only** — put DB clients/secret
  helpers there. Secrets via `$env/static/private` (or `dynamic/private`); only `PUBLIC_`-prefixed
  values reach the browser. `import type` safely crosses the boundary.

### Desktop (Tauri, adapter-static SPA)
- **SSR off, no prerender** (`export const ssr = false; export const prerender = false`) — Tauri
  has no server; SPA mode runs `load` in the webview where `invoke` works. No `*.server.ts` /
  `+server.ts` in the desktop app.
- **One typed IPC client layer** wrapping `invoke` (`$lib/ipc.ts`); never scatter raw
  `invoke('cmd', …)` calls. Mirror Rust command signatures in TS. **Always test `tauri build`**,
  not just `dev`.

### Project conventions
- **Design system:** shadcn-svelte components + design-token CSS variables. **No hardcoded
  colors.** **Icons must come from `@recast/icons`** (Tabler re-exports under Lucide-style names)
  or `@recast/icons/ai` (Phosphor duotone accents for AI-touchpoint surfaces only).
  Direct imports from `@tabler/icons-svelte`, `@lucide/svelte`, or `@phosphor-icons/*` are
  blocked by Biome's `noRestrictedImports`. See `packages/icons/README` for the curated list.
- **bits-ui wrappers** (Dialog/Sheet/Dropdown/Select/Popover/HoverCard) must default
  `preventScroll={false}` — the upstream default leaks `pointer-events:none` on `<body>` in the
  Tauri build.
- `.svelte.ts` for rune logic, `.svelte` for components. **`svelte-check` clean is a merge gate.**

### Anti-patterns to ban
`$effect` to sync state · mutating non-`$bindable` props · server-side module-global request state
· deep reactive `$state` for large blobs (use `$state.raw`) · `$:` / `export let` /
`createEventDispatcher` / `<slot>` / `on:event` / `$app/stores` in new code · raw scattered `invoke`.

### Media pipeline & non-linear-editor performance contract
The desktop video editor's preview and the apps/web conversion tools read media through
`@recast/media`. The full contract — performance budgets, curated web.dev guides,
implementation rules, browser API surface — lives in `packages/media/REQUIREMENTS.md`. The
migration sequence and per-milestone testing gates live in `packages/media/PLAN.md`. These
rules are non-negotiable:

- Decode, demux, and codec configuration MUST run inside a Web Worker. Never on the main thread.
- `AudioWorklet` is the only acceptable audio path for sample-aligned scheduling. The legacy
  `apps/desktop/src/lib/playback/audio-engine.ts` exists as a fallback during the migration
  window; remove the fallback only after a milestone's testing confirms the new path is stable.
- Compositing MUST target the main thread unless an OffscreenCanvas profile proves otherwise; use
  `transferControlToOffscreen` to migrate, never a parallel canvas.
- Frame-to-glass p95 ≤ 16.7 ms during playback. Cut-cross latency p95 ≤ 250 ms. Both are tracked
  by `packages/media/test/perf/budgets.test.ts` and `packages/media/test/perf/cut-jump.test.ts`.
- All `VideoFrame`s crossing the worker boundary are owned by the consumer side; the producer side
  MUST NOT close them until a release message returns. This is the same invariant as the current
  `apps/desktop/src/lib/playback/webcodecs-source.ts:22`.
- Bundle: `@recast/media` ≤ 80 KB gz desktop, ≤ 150 KB gz web. Direct imports only; tree-shake
  relies on this.
- IndexedDB-backed decoded-frame cache: ≤ 2 GB cap (user-configurable in Settings; default 2 GB),
  LRU by recency × bytes, evicted during the `idle` callback where possible (web.dev INP guidance).
- No `mediabunny` import outside `packages/media`. Consumers go through `@recast/media`'s high-
  level API. Worker modules that need the raw classes import `@recast/media/mediabunny` — never
  the main barrel, which must stay tree-shakable.
- The Rust export pipeline stays untouched. `EnqueueExportRequest` IPC payload is byte-stable.
- Every PR in this surface area must read `packages/media/REQUIREMENTS.md §4` (curated web.dev
  guides) before opening.
- The preview decode engine lives in `packages/media/src/playback/`, not in `apps/desktop`. It is
  pure Web-platform code (Worker + WebCodecs + OffscreenCanvas) and must stay Tauri-free so
  `apps/web` can use it.
- Any decoded frame the cache stops referencing MUST be `close()`d. A `VideoFrame` holds a GPU
  surface the GC will not reclaim promptly; an uncapped frame Map is a memory leak, not a cache.

---

## 5. Server-side TypeScript (web app: SvelteKit + Hono + Drizzle + Zod)

> Three current realities that reshape old advice: **(1)** Vercel **Fluid Compute is default** —
> Node is the right default, and module-scope state is now a *cross-request leak*, not just
> non-durable. **(2)** **Zod 4** lives under `zod/v4` with stricter defaults. **(3)**
> **postgres.js behind a transaction pooler requires `prepare: false`.**

### TypeScript rigor
- **`strict: true` is the floor.** Add `noUncheckedIndexedAccess` (biggest real-world bug catcher
  for arrays/records/`process.env`), `exactOptionalPropertyTypes`, `noImplicitOverride`,
  `verbatimModuleSyntax`, `isolatedModules`, `module: "nodenext"`.
- **`satisfies`, not `: Type`,** for config/route/const maps (validate without widening).
- **`as const` unions, not `enum`.** **`unknown` at boundaries; never `any`; never `as` to lie.**
  Model domains with discriminated unions + exhaustive `switch`. `catch (e)` is `unknown` — narrow
  with `instanceof Error`.

### Validation (Zod 4)
- **Parse-don't-validate at every edge** — body/query/params/headers/**env**/3rd-party responses.
  **Schema is the single source of truth:** `type X = z.infer<typeof X>`, never a parallel interface.
- **`.parse()` at startup** (fail boot on bad env), **`.safeParse()` in handlers** (→ 422 with
  `z.treeifyError(error)`). **Share one schema module** so client validates responses with the
  same definition the server validates requests.
- **Zod 4 deltas:** import from `zod/v4`; top-level `z.email()/z.uuid()/z.url()` (string-method
  forms deprecated); unified `error` param (drop `message`/`invalid_type_error`/`errorMap`);
  `z.strictObject()` to reject unknown keys; `z.record(k, v)` needs two args; `.format()`/`.flatten()`
  → `z.treeifyError()`; audit `.default()` (now output-side; use `.prefault()` for input-side).

### API design (Hono / SvelteKit endpoints)
- **`zValidator` per-route, not via `app.use`** (type inference only merges in the route's chain;
  `c.req.valid('json')`). **Override the validator's error** — never ship a raw `ZodError`.
- **Status as literals** (`c.json(body, 201)`) for typed-client inference. Build `AppType` by
  chaining off one `Hono()` instance; export `type AppType = typeof app`. For a separate frontend
  use `hcWithType`, with `strict:true` + identical Hono versions on both sides.
- **One error envelope everywhere** (`{ error: { code, message } }` or RFC 9457 problem+json).
  Precise status mapping (400 unparseable, 422 invalid, 401 vs 403, 429 + `Retry-After`).
- **SvelteKit routing:** `+server.ts` for a few endpoints; mount a full Hono app under a catch-all
  (`export const fallback = ({ request }) => honoApp.fetch(request)`) when you want
  middleware/RPC; form actions when there's no non-browser client. On Vercel, `export default app`
  (zero config).

### Error handling
- **`Result`/`{ ok, error }` for expected domain failures; throw for the exceptional.** Classify
  operational (handle → 4xx/5xx) vs programmer (a bug — end the invocation, don't keep a poisoned
  instance). Custom `AppError` with `code` + `statusCode` + `isOperational`.
- **Funnel through one handler** (Hono `app.onError`; SvelteKit `handleError`). **SvelteKit 2:
  call `error(status, …)`, do NOT `throw error(…)`.** **Log full error+stack server-side; never
  leak internals to clients.** No empty `catch {}`; `await` every promise (enable
  `no-floating-promises`).

### Database (Drizzle + postgres.js on Vercel)
- **⚠ `prepare: false`** when connecting through a transaction-mode pooler (postgres.js
  auto-prepares; the pooler breaks prepared statements silently). **Module-scope singleton
  client**, small `max` (1–5), short `idle_timeout`; call `attachDatabasePool()` from
  `@vercel/functions` under Fluid. App connects via the **transaction pooler (6543)**; **migrations
  via the direct connection (5432)**.
- **Schema is source of truth:** `drizzle-kit generate` → commit SQL → `migrate` in a **CI/CD
  step, never at function runtime**. `push` is dev-only. **Tagged-template `sql\`…${v}\``** auto-
  parameterizes — never concatenate; `sql.raw/unsafe` is allowlist-only.
- **Relational queries (`db.query…{ with }`) to kill N+1.** `db.transaction()` for atomicity, kept
  short. **All queries live in a repository/data layer** (the only place importing `db`); expose
  domain methods, not ORM passthroughs.

### Security
- **Authz at every boundary, per object — authn ≠ authz** (BOLA/IDOR is API risk #1). Scope queries
  by owner at the data layer (`WHERE id=$1 AND user_id=$2`), don't fetch-then-check. **Never trust
  client-supplied identity** — derive from the validated server session.
- **better-auth:** let it manage cookies (`httpOnly`+`Secure`+`SameSite=Lax`); set `trustedOrigins`
  explicitly; **never `disableCSRFCheck`**; validate the session server-side on every protected
  request. **Rate-limit with durable storage** (DB/Redis/Upstash), not in-memory (serverless
  instances don't share memory).
- **Keep SvelteKit's origin CSRF check on; add Hono `csrf()` to the mounted API separately.** CORS:
  exact-origin allowlist when `credentials: include` (never `*` + credentials). `hono/secure-headers`
  + `config.kit.csp`. **SSRF:** allowlist hosts for any outbound fetch of user-supplied URLs.
  **Storage keys are untrusted** — reject `..`/absolute paths, scope under a per-user prefix.
  **Signed URLs** so clients talk to S3/R2 directly (authorize + constrain key/content-type before
  signing; short expiry); never proxy large files through the function.

### Serverless / Vercel
- **Default Node runtime + Fluid Compute; Edge only for latency-critical, Web-API-only, DB-less
  work** (Edge has no TCP → postgres.js won't run). **All in-memory state is ephemeral AND a
  cross-user leak under Fluid** — module scope is for connections only; durable/shared state →
  Redis/KV. `waitUntil()` only for cheap fire-and-forget. **Cron in `vercel.json` guarded by
  `CRON_SECRET`.** Set `maxDuration` per function; co-locate function + DB region.

### Architecture & observability
- **Thin handler → service → repository.** Handlers only parse → validate → call service → format.
  **Pass plain validated DTOs into services, never the `Context`/`RequestEvent`** (so the same
  service runs from a route, an action, or a cron). **Map to an explicit DTO at the boundary —
  never `c.json(dbRow)`** (couples API to schema, leaks fields). Shared contracts (Drizzle tables +
  Zod validators + DTO types) live in a `packages/*` workspace package, imported by both apps.
- **Structured JSON logging (Pino) to stdout — no `console.log` soup.** **No Pino transports /
  `pino-pretty` in serverless prod** (worker-thread flush loss). **Request/correlation ID on every
  line**; **never log PII/secrets/tokens/full bodies** (use `redact`). Sentry on SvelteKit hooks
  with `tracesSampleRate ≈ 0.1` in prod.

---

## 6. Definition of done

A change is done when, for every area it touched:

- [ ] Behaviour-preserving (or an explicitly-intended, stated change).
- [ ] Boundaries validated; errors surfaced (not swallowed); no secrets leaked client-side.
- [ ] Types intact end-to-end; serialized contracts updated on **both** sides in the same change.
- [ ] Gates green: `fmt`/`clippy` (Rust), `svelte-check`, `vitest`/`cargo test`, `pnpm check`/`build`.
- [ ] (Rust) all target_os trees compile; new threads/processes have RAII teardown; ffmpeg spawns
      are silent; heavy commands stay off the UI thread.
- [ ] (Frontend) no banned Svelte 4 idioms; design tokens only; icons via `@recast/icons`
      (Tabler) or `@recast/icons/ai` (Phosphor duotone — AI touchpoints only);
      `tauri build` tested if desktop runtime behaviour changed.
- [ ] (Server) Node+Fluid assumptions held; no module-scope request state; DB through the repo
      layer with `prepare:false`; authz checked per object.
- [ ] Small commits, Conventional Commit messages, on a branch; maintainer owns the merge.

---

*Keep this file current. When a convention changes, update the rule here in the same PR so the
guidance never drifts from the code.*
