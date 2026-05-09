# Recast Desktop — Design System

The desktop app for Recast: a Tauri v2 + Svelte 5 + Rust recorder/editor that
runs offline-first across macOS, Windows, and Linux. This document captures the
visual language, component conventions, and motion vocabulary so every screen
and webview lands consistently.

> **Stay opinionated.** Recast is a tool for people who'd rather ship than fiddle.
> The desktop chrome should feel the same way: dense, calm, native-feeling, and
> never overwrought.

The desktop language differs from the marketing site by intent. The web doc
([apps/web/DESIGN.md](../web/DESIGN.md)) optimizes for first-impression rhythm —
hero headlines, atmospheric backgrounds, big section breaks. The app optimizes
for return-visit comfort — high information density, small type, glass surfaces
that recede so the user's content is the focal point.

---

## Audience

Same audience as the marketing site — solo founders, indie hackers, product
engineers — but in a different mode. They're already using the product. The UI
should reward muscle memory: predictable shortcuts, dense lists, quick toggles.
Avoid teaching microcopy on every surface; rely on the fact that returning users
already know what the buttons do.

---

## Voice (in-app)

| Do | Don't |
| --- | --- |
| Short labels, sentence case. ("New profile", "Output directory") | Title Case ("New Profile") or ALL CAPS button labels. |
| Imperative microcopy that names the outcome. ("Save what to capture.") | Marketing sentences inside settings ("Empower your workflow…"). |
| Toast messages that name the artifact. ("Deleted *Presentation*.") | Generic confirmations ("Success", "Done"). |
| Tooltips on icon-only buttons, with the keyboard shortcut included. | Bare icon-only buttons with no `aria-label` or tooltip. |

Do not echo the marketing voice inside the app. The headline pattern *"Demos
that look [outcome]"* belongs on the landing page, not on the recordings page.

---

## App shape

The desktop app is a SvelteKit SPA shell hosting **multiple Tauri webviews**:

| Window | Route | Notes |
| --- | --- | --- |
| Main | `/(app)` group | Sidebar + titlebar shell. Navigation lives here. |
| Recording panel | `/panel` | Floating, transparent, draggable. ~44px tall. |
| Source selector | `/select` | Modal-style picker over a transparent window. |
| Device picker | `/device-picker?type=mic\|camera` | Popover-sized, transparent, opened from panel. |
| Region selector | `/select-area` | Full-screen overlay for region capture. |
| Camera preview | `/camera-preview` | Floating webcam bubble, alwaysOnTop on macOS/Windows. |
| Editor | `/editor/:slug` | Full-window, no sidebar. |

The transparent secondary windows (`panel`, `select`, `device-picker`,
`camera-preview`) take a different background treatment — they paint their
own surface with a margin of transparent padding around it. Routes in
`TRANSPARENT_ROUTES` (see [+layout.svelte](src/routes/+layout.svelte)) opt into
`bg-transparent` instead of `bg-background`.

Plugins that inject JS APIs (`tauri_plugin_os`, `tauri_plugin_dialog`) **must**
be registered on the `Builder` before any window is created — registering them
inside `setup()` causes a hard hang on the boot splash. See
[lib.rs](src-tauri/src/lib.rs).

---

## Color & Theme

Tokens live in [`@recast/design`](../../packages/design/src/index.css). Always use
CSS variables — never hardcode colors. The desktop scope adds a few utilities
in [app.css](src/app.css) but does not override the token palette.

| Token | Use |
| --- | --- |
| `--background` / `--foreground` | App background, primary text. |
| `--card` / `--card-foreground` | Glass surfaces (settings rows, profile cards, panel body). |
| `--popover` / `--popover-foreground` | Dropdowns, tooltips, popovers, **toasts**. |
| `--primary` (lime) | Active state, default-profile star, accent icons. **Never** as a full surface fill. |
| `--muted-foreground` | Secondary copy, microlabels, inactive icons. |
| `--border` / `--border-subtle` | Hairlines, ring-insets. Always at 40–60% alpha. |
| `--destructive` | Stop-recording button, delete actions, error toasts, validation errors. |
| `--success` | Successful save toasts, "ready" device validation. |
| `--warning` | Default-profile badge, missing-device fallback toast. |
| `--info` | Informational toasts (e.g. "no slots free"). |

### Status mapping

Status colors are **only** for status — never decorative. The mapping is fixed:

| Outcome | Token | Toast variant | Example |
| --- | --- | --- | --- |
| Saved / created / applied | `--success` | `toast.success` | "Profile created", "Output directory updated" |
| User-initiated removal | `--destructive` | `toast.success` (not error — the action succeeded) | "Deleted *Presentation*" |
| Action failed | `--destructive` | `toast.error` | "Recording failed: …" |
| Soft warning, fallback applied | `--warning` | `toast.warning` | "Camera *Yeti* unavailable — using *Default*" |
| Capacity / blocking info | `--info` | `toast.info` | "All N capability combinations are in use" |
| Recording in progress | `--destructive` | n/a (live UI) | Stop button, recording dot |

### Dark mode

The dark `--primary` is a highly saturated lime (`oklch(92.249% 0.234 …)`). When
mixing primary into surfaces, **stay below 8%** in dark mode. Prefer
`color-mix(in srgb, var(--color-foreground) 4-6%, transparent)` for ambient
glows. Always test new components in both modes; the cards/panels look
identical at a glance but dark-mode primary mixes are aggressive.

---

## Typography

- **Sans:** `Geist Variable`. Tight tracking (`tracking-tight`), `font-feature-settings: "ss01"`.
- **Mono:** `Geist Mono Variable`. Use for paths, version numbers, timer readouts.

### Scale (smaller than web by design)

The desktop app is a high-information surface. Most text sits below 14px so the
window holds more content without scrolling.

| Use | Size | Notes |
| --- | --- | --- |
| Page hero h1 | `text-[28px] md:text-[32px]`, `font-semibold`, `leading-tight`, `tracking-tight` | Always `text-balance`, gradient `bg-clip-text` from `foreground` to `foreground/55`. |
| Section eyebrow | `text-[11px]`, `font-bold`, `uppercase`, `tracking-[0.15em]`, `text-muted-foreground/70` | Always paired with a one-line description in `text-[11px] text-muted-foreground/80`. |
| Card title | `text-[13.5px]`, `font-semibold`, `tracking-tight` | Profile cards, settings rows. |
| Card secondary | `text-[10.5–11px]`, `text-muted-foreground` | Capability summaries, hint text. |
| Body / hero subhead | `text-[12.5px]`, `leading-relaxed`, `text-muted-foreground` | Page intros under the hero. |
| Microlabel (uppercase) | `text-[10px]`, `font-bold`, `uppercase`, `tracking-[0.15em–0.18em]` | Eyebrow chips, dialog field labels. |
| Panel chrome | `text-[12–13px]`, `font-semibold`, `tracking-tight` | Recording panel, titlebar text. |
| Timer readout | `font-mono`, `text-[13px]`, `tabular-nums`, `font-semibold` | Live recording duration. |

Always `text-balance` headings. Always `text-pretty` body paragraphs.

---

## Layout

### Page rhythm

Top-level routes inside the `(app)` group follow this pattern:

1. **Eyebrow chip** — pill with a Lucide icon + section name. Sits at the top
   of the hero. Fly-in on mount.
2. **Hero h1** — gradient-clipped, balanced, two short lines max. May include a
   right-aligned action button (e.g. "New profile").
3. **Hero supporting copy** — single line, `text-muted-foreground`. Optionally
   appends meta info (e.g. "5 of 18 combinations free.").
4. **Search / filter bar** *(if listing)* — 12-tall pill with a Lucide icon.
5. **Sections** — each with an uppercase microlabel + one-line description in
   the left margin, content card on the right. Use `gap-8` between sections.

Reference: [profiles/+page.svelte](src/routes/(app)/profiles/+page.svelte),
[settings/+page.svelte](src/routes/(app)/settings/+page.svelte).

### Container widths

- **Settings, Profiles, What's New:** `max-w-5xl`, `px-6`, `py-10`.
- **Recordings list:** `max-w-6xl` for the grid.
- **Editor:** full window, no container.

### Section dividers

`border-t border-border/40` only when explicitly needed. Prefer ample whitespace
(`gap-8` / `mt-8`) over horizontal rules.

---

## Components

### Glass surfaces

The canonical surface pattern in the desktop app:

```html
<div class="rounded-xl border border-border/60 bg-card/70 shadow-(--shadow-craft-inset) backdrop-blur">
```

Variants:

| Class combo | Use |
| --- | --- |
| `bg-card/70 + border-border/60 + shadow-craft-inset + backdrop-blur` | Settings rows, profile cards, dialog content. |
| `bg-card/95 + border-border/60 + ring-1 ring-foreground/5 + backdrop-blur-3xl` | Recording panel — opaque enough to be readable over any desktop background. |
| `bg-popover + ring-1 ring-border/60 + shadow-craft-inset-strong` | Dialogs, popovers (managed by `bits-ui` defaults). |
| `bg-card/40 + border-dashed` | Empty state placeholders. |

**Never** use a flat fill. Glass requires a non-trivial background to read; the
`(app)` shell provides the noise gradient. Transparent routes (panel etc.) sit
on the desktop directly, so their surface alpha is bumped to `/95`.

### Eyebrow chip

```svelte
<span class="inline-flex w-fit items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.15em] text-muted-foreground/80 backdrop-blur">
  <Icon class="size-3 text-primary" />
  Profiles
</span>
```

Always lead a top-level page with an eyebrow chip, paired icon tinted `--primary`.

### Buttons

Use `Button` from `@recast/ui/button`. Common patterns:

- **Page-level primary action** (e.g. "New profile"): `size="sm"` `class="h-9 gap-1.5"` with leading Lucide icon at `size={13}` and trailing `<Kbd>⌘N</Kbd>`.
- **Settings inline action** (e.g. "Change"): `variant="secondary"` `size="sm"` `class="h-9 gap-1.5"`.
- **Card row trailing action** (e.g. "Edit" on a profile card): `variant="ghost"` `size="xs"` `class="h-6 gap-1 px-1.5 text-[10.5px] text-muted-foreground"`.
- **Destructive action**: `variant="destructive_soft"` for soft-destructive (delete), `variant="destructive"` only for top-level "Stop Recording" / "Delete forever".
- **Icon-only**: `variant="ghost"` `size="icon-sm"`, **always** wrap in a `Tooltip` with the keyboard shortcut, never rely on the bare `title=` attribute.

### Disabled buttons in tooltips

A native disabled `<button>` swallows pointer events — wrap it in a `<span>`
inside `Tooltip.Trigger`'s `child` snippet so hover still fires:

```svelte
<Tooltip.Root>
  <Tooltip.Trigger>
    {#snippet child({ props })}
      <span {...props as Record<string, unknown>}>
        <Button disabled={isFull}>…</Button>
      </span>
    {/snippet}
  </Tooltip.Trigger>
  <Tooltip.Content>Why it's disabled</Tooltip.Content>
</Tooltip.Root>
```

### Toaster

Mounted once in [+layout.svelte](src/routes/+layout.svelte). Use the
`@recast/ui/sonner` wrapper, which themes Sonner with our tokens:

```svelte
<Toaster position="top-center" />
```

**Do not** pass `richColors` — it overrides the popover-token theming with
Sonner's stock saturated palette and clashes with the muted desktop UI. Variant
mapping uses our tokens (`--success`, `--destructive`, `--warning`, `--info`)
internally; if you need a status hue, call the matching `toast.success` /
`toast.error` / `toast.warning` / `toast.info`. Never hand-style toasts via
`className` to inject a hex.

### Dialog rhythm

Profile editor and similar dialogs follow a consistent header / body / footer:

- **Header** (`px-5 py-4`, `border-b border-border/40`): `Dialog.Title` at `text-[14px] font-semibold` + `Dialog.Description` at `text-[11px] text-muted-foreground`.
- **Body sections** are separated by `border-b border-border/30` rather than
  internal padding.
- **Footer** (`bg-muted/30`, `px-3 py-2.5`): destructive action on the left,
  Cancel + Save on the right. The save button shows `<Kbd>⌘↵</Kbd>` if
  `Cmd+Enter` submits.

### List rows

Profile cards, recordings cards, exports cards all share the same chrome:

```html
<div class="group/card relative flex flex-col gap-3 rounded-xl border border-border/50 bg-card/70 p-4 shadow-(--shadow-craft-inset) backdrop-blur transition-all duration-200 hover:-translate-y-0.5 hover:shadow-craft-sm">
```

Hover lifts by 2px and adds a subtle elevation shadow — that's the signal a row
is interactive. Don't apply this on rows that aren't clickable.

### Recording panel

The panel is a unique surface — `~44px` tall horizontal pill, `bg-card/95`,
heavy `backdrop-blur-3xl`. Treat it as **buttons only**. Avoid text labels
beyond the source name and the timer; everything else is icon + tooltip.

If you need to add controls, go vertical with a popover (e.g. profile switcher)
rather than widening the bar.

---

## Motion

The desktop app uses two ease curves:

- **`cubicOut`** (svelte/easing) — `cubic-bezier(0.33, 1, 0.68, 1)`. Default for in/out transitions on page mount, hero reveals, list staggers.
- **`cubic-bezier(0.16, 1, 0.3, 1)`** — used for `boot-pop`, dialog opens, and any "spring-into-place" feel. Heavier on the back end.
- **`cubic-bezier(0.625, 0.05, 0, 1)`** — `CRAFT_EASE` from `@recast/ui/utils`. Use for craft-block animations and overlay enter/exit (parity with web).

Default durations:

| Use | Duration |
| --- | --- |
| Hover state changes | `duration-200` |
| Page hero fly-in | `duration: 320` |
| Card stagger | `duration: 240`, `delay: i * 40` (cap at 240ms) |
| Dialog open (bits-ui default) | `duration-200` enter / `150` exit |
| View Transitions page swap | `280ms`, `cubic-bezier(0.32, 0.72, 0, 1)` (Apple SF curve) |

`reduced-motion` is honored globally by [app.css](src/app.css) — never override.

---

## Icons

**Lucide only.** `@lucide/svelte` is the only sanctioned icon library in the
desktop app. The shared UI package historically pulls some Tabler icons in
its Sonner wrapper; that is being migrated. Do not introduce new Tabler/Phosphor/Heroicons usage.

Sizing:

| Context | Size |
| --- | --- |
| Page action button (`size="sm"`) | `size={13}` |
| Card primary | `class="size-4"` |
| Card secondary / chip | `class="size-3"` or `class="size-3.5"` |
| Eyebrow chip | `class="size-3 text-primary"` |
| Toaster variant | `class="size-4"` |
| Recording panel | `size={12–14}` |

Always pass `strokeWidth={2}` for panel-tier icons (slightly chunkier reads
better at small sizes); leave default elsewhere.

---

## Keyboard

Every page that has primary actions should expose them as ⌘/Ctrl shortcuts.
Patterns:

- ⌘N: New (whatever the page lists).
- ⌘R: Edit/rename current row (only when one is focused).
- ⌘D: Duplicate.
- ⌘1..⌘8 (or N): Switch profile.
- ⌘↵ (Cmd+Enter): Save in dialogs.
- Escape: Cancel (managed by `Dialog.Root`).

Always render the shortcut in a `<Kbd>` element inside the button label or
tooltip — discoverable shortcuts only.

---

## Persistence

The desktop app stores user state in two places:

- **Tauri config** (Rust-managed, JSON on disk): output directory, last source.
  Survives reinstalls per-user. Update via `getOutputDir / setOutputDir`-style
  IPC commands.
- **localStorage**: theme (`mode-watcher-mode`), editor window behavior
  (`recast-editor-window`), recording profiles (`recast-recording-profiles`),
  profile-system enabled flag (`recast-profiles-enabled`). Browser-resettable;
  never store anything sensitive here.

Always migrate forward when extending a localStorage schema — read the old
shape, populate missing fields with sensible defaults, write back the new
shape. Never throw on a parse failure; reset to the empty-state instead.

---

## Dos and Don'ts

**Do**

- Use markdown link syntax (`[file.svelte](path)`) when referencing code in docs.
- Use Lucide icons only. Match the size table above.
- Reference design tokens via Tailwind utilities (`bg-primary`, `text-muted-foreground`).
- Reach for the canonical glass surface (`bg-card/70 + border-border/60 + shadow-craft-inset + backdrop-blur`) before inventing a new container.
- Wire keyboard shortcuts and surface them in tooltips/labels.
- Test new screens in both light and dark mode.
- Use `toast.success / .error / .warning / .info` to color-code status.

**Don't**

- Hardcode hex/rgb values. Use CSS variables and `color-mix()` in `srgb`.
- Pass `richColors` to `Toaster`. Use the design-token wrapper.
- Use a different icon library (no Tabler, no Phosphor, no Material).
- Add disabled `<button>`s without wrapping them in a tooltip span — hover gets eaten otherwise.
- Echo marketing voice ("Empower your workflow") inside the app.
- Use full primary fills on large surfaces in dark mode — the saturated lime overpowers everything.
- Register Tauri JS-injecting plugins inside `setup()` — boot splash hangs.
- Stack absolutely-positioned cards inside fixed-height containers; use a grid.

---

## Cross-references

- Marketing voice and section rhythm: [apps/web/DESIGN.md](../web/DESIGN.md).
- Token source: [packages/design/src/index.css](../../packages/design/src/index.css).
- Shared components: [packages/ui/src/components/ui/](../../packages/ui/src/components/ui/).
