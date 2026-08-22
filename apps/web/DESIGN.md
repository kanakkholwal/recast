# Recast Web — Design System

The marketing site for Recast: a border-first, near-monochrome editorial surface
with one accent hue and a small vocabulary of coloured feature tags.

> **Borders, not depth.** A container is a 1px hairline and a radius. If a surface
> needs blur or a drop shadow to read, it needs better spacing instead.

This document replaces the glass-and-atmosphere system that preceded it. That
system defined containers with `backdrop-filter: blur(24px)` plus four-layer
shadows, then diluted its own hairline into `/30`–`/70` opacities that measured
**1.03:1** and drew nothing. Structure now comes from one border used at full
strength, everywhere.

Scope: `[data-site="marketing"]`, set on `<html>` by
[+layout.svelte](src/routes/+layout.svelte) via `isMarketing()` in
[layout.logic.ts](src/routes/layout.logic.ts). `/dashboard`, `/admin`, `/share`,
`/onboarding` and `/playground` opt out and keep the tonal card-over-canvas
system, so the two can coexist.

---

## Audience

1. **Solo founders** — building, demoing, and pitching weekly.
2. **Indie hackers** — launch videos, changelog clips, Twitter cuts.
3. **Product engineers** — demoing PRs, explaining bugs, documenting APIs.

If a sentence reads as if it targets an enterprise procurement team, rewrite it.

---

## Voice

| Do | Don't |
| --- | --- |
| Direct, opinionated, founder-to-founder. | Marketing-speak ("solutions", "leverage"). |
| Concrete verbs ("ship", "record", "trim"). | Vague abstractions ("empower", "transform"). |
| Punchy two-line headlines. | Long paragraph hero copy. |
| Emphasise outcomes (looking expensive, shipping fast). | Feature lists with no "so what?". |

**Through-line:** Record → Polish → Share.

---

## Colour

Tokens live in [`@recast/design`](../../packages/design/src/index.css); the
marketing overrides live in [app.css](src/app.css). Everything is oklch. Never
hardcode hex.

### Surfaces & ink

| Role | Light | Dark | Token |
| --- | --- | --- | --- |
| Canvas | `oklch(100% 0 0)` | `oklch(14.5% 0 0)` | `--background` |
| Card | `oklch(100% 0 0)` | `oklch(14.5% 0 0)` | `--card` |
| Paper (tonal band) | `oklch(97% 0 0)` | `oklch(18.5% 0 0)` | `--paper` → `bg-paper` |
| Hairline | `oklch(92.2% 0 0)` | `oklch(30% 0 0)` | `--border` → `border-border-low` |
| Emphasis border | `oklch(87% 0 0)` | `oklch(42% 0 0)` | `--border-emphasis` → `border-border-strong` |
| Text | `oklch(20.5% 0 0)` | `oklch(96% 0 0)` | `--foreground` |
| Muted text | `oklch(55.6% 0 0)` | `oklch(68% 0 0)` | `--muted-foreground` |

Light-mode canvas is pure white and cards are pure white. They are told apart by
the hairline, not by tone. That is the whole idea.

**Dark mode mirrors that, and the mirror is the rule.** Card equals canvas in
dark too, so a card on a `bg-paper` band reads by its border, not by a lift.
Card sitting *between* canvas and paper (the old `17.5%`) made every panel on a
band look sunken, which is what "dark mode feels muddy" actually was. The three
tag hues also get lifted in dark (`74-75%` L) — the light-mode values sit at
54-64% and go to mud on a 14.5% canvas.

### Accent

`--primary` is `oklch(0.52 0.20 264)` on light, `oklch(0.70 0.155 264)` on dark.
It measures **5.52:1** on white; the reference system's `#2563eb` measures
**5.17:1** and sits at `oklch(54.6% 0.2152 262.9)`. The two are the same colour
within measurement noise, so the existing token stays — it is already CVD-validated
against `--destructive` and documented in the design package.

**`--primary` is reserved for:**

1. The single main CTA on a view.
2. Links inside body copy.
3. The sidebar active-route indicator.
4. The primary data series in a chart.
5. Selection and drag-drop targets.
6. Toggle "on" states.
7. Plan / upgrade affordances.
8. Focus rings and active-input borders (`--ring` is `--primary`).

**Never** a decorative tint. The old landing page carried 31 `bg-primary` dots as
eyebrow bullets; they are now neutral or a feature-tag hue.

The primary *filled* action is near-black (`variant="dark"` = `bg-foreground`),
not blue. Blue highlights, black commits.

### Feature-tag accents

One hue per tag, never two on one component. Used on the hero step shelf and on
section eyebrows for the three spine steps.

| Tag | Value | Token |
| --- | --- | --- |
| Tangerine (Record) | `oklch(64.6% 0.1943 41.1)` | `--color-tag-tangerine` |
| Lavender (Polish) | `oklch(54.1% 0.2466 293)` | `--color-tag-lavender` |
| Green (Share) | `oklch(62.7% 0.1699 149.2)` | `--color-tag-green` |

Icons render **duotone**: `fill="currentColor"` plus `[fill-opacity:0.2]`, so the
glyph carries a translucent fill under a full-strength stroke in one hue. No
tinted background tile behind the icon.

### Colour ratio (60/30/10)

| Share | Role | Tokens |
| --- | --- | --- |
| **60%** | Canvas. | `--background` |
| **30%** | Structure: hairlines, paper bands, muted text. | `--border`, `--paper`, `--muted-foreground` |
| **10%** | Accent: `--primary` plus the three tag hues. | see the reserved list above |

### Contrast floors

Non-negotiable, and cheaper to check than to relitigate:

| Thing | Floor |
| --- | --- |
| Body text on its surface | 4.5:1 |
| Focus ring, control boundary, meaningful icon | 3:1 |
| Two controls distinguished by colour | 3:1 **luminance**, never hue alone |

Four measured failures were repaired in this pass. Do not reintroduce them:

| Element | Was | Now |
| --- | --- | --- |
| h2 second line `text-foreground/45` (×6) | `#959595` **2.86:1** | `text-muted-foreground` 4.6:1 |
| Final CTA h2 `text-foreground/40` | `#a0a0a0` **2.49:1** | `text-muted-foreground` 4.6:1 |
| Email placeholder `text-muted-foreground/70` | `#969ba5` **2.68:1** | `text-muted-foreground` 4.6:1 |
| `border-border-low/30` | ~**1.03:1** | `border-border-low` (full strength) |

**Never fade a text token with an opacity modifier.** If copy should be quieter,
it takes `text-muted-foreground`. Opacity on text is how the four failures above
happened.

---

## Typography

- **Display (h1, h2):** Satoshi, weight **700**, letter-spacing **-0.01em**.
- **Subheads (h3):** Satoshi, weight **500**, letter-spacing **-0.008em**.
- **Everything else:** Inter — body, UI labels, h4–h6 (weight 600, `-0.011em`).
- **Mono:** Geist Mono, for code and architecture docs. **Not on the marketing
  pages** — file names, timecodes and readouts inside product mocks use Inter
  with `tabular-nums`.

Satoshi ships 400 / 500 / 700 — there is no 600. 500 read too light at display
sizes next to the reference, so display type is Bold; at body size 700 is too
heavy, so h3 takes 500. Every heading on a marketing page is on the display
face, and carries `font-display` in the markup as well as the base rule, so it
is visible where the heading is written.

Satoshi is **not on Fontsource** — no `@fontsource/satoshi` package exists. The
ITF-Free-Font-License woff2 files are vendored in
[static/fonts](static/fonts) and declared with `@font-face` in
[app.css](src/app.css). Inter and Geist Mono come from Fontsource as normal.
Licence terms are recorded in [THIRD-PARTY-LICENSES.md](../../THIRD-PARTY-LICENSES.md).

### Scale

Nine steps. Line height rides along via Tailwind v4's `--text-*--line-height`
pairing, so `text-body` sets both and no call site needs a `leading-` utility.

| Token | Size | Line height |
| --- | --- | --- |
| `text-caption` | 11px | 1.5 |
| `text-body-sm` | 14px | 1.43 |
| `text-body` | **16px** | 1.5 |
| `text-body-lg` | 18px | 1.56 |
| `text-subheading` | 20px | 1.4 |
| `text-heading-sm` | 24px | 1.33 |
| `text-heading` | 30px | 1.25 |
| `text-heading-lg` | 36px | 1.11 |  ← section h2
| `text-display` | 48px | 1.04 |  ← hero h1 only
| `text-display-lg` | 60px | 1.02 |

16px is the canonical body size. **No ad-hoc `text-[Npx]`.** The page previously
carried 13 arbitrary sizes including 7px, 8px, 9.5px, 10.5px, 11.5px and 12.5px.

Use `text-balance` on every headline, `text-pretty` on every body paragraph.

### Eyebrows

Sentence-case pills, not uppercase micro-labels. The old system had 140
`uppercase` and 117 `tracking-[0.Xem]` declarations; at 11px that costs legibility
and, repeated, reads as a tic. `<SectionHeader eyebrow="…">` renders the pill.

---

## Shape

Five radii. Nothing else.

| Element | Value | Utility |
| --- | --- | --- |
| Tags, badges, pills | 9999px | `rounded-full` / `.pill` |
| Inputs | 6px | `rounded-sm` |
| Buttons | 8–12px | `rounded-lg` |
| Cards | 12px | `.surface` |
| Feature surfaces, mockups | 16px | `.surface-lg` / `.mockup-frame` |

The one exception is the notched shelf's wings, which are an SVG path rather
than a radius. See **NotchedShelf** below.

**Deviation from the reference:** buttons sit at 12px (`--radius-lg`) rather than
8px, because the radius scale is derived from `--radius` and an 8px button step
would add a sixth value for no gain.

---

## Elevation

Borders define containers. Shadows are allowed in exactly three places:

| Use | Value |
| --- | --- |
| Filled button lift | `--shadow-craft-sm` = `0 1px 2px 0 rgba(0,0,0,.05)` |
| Selected state in the hero shelf | `--shadow-craft-sm` |
| Product mockup frame | `--shadow-craft-floating` = `0 0 0 4px` foreground @6% |

Everything else is a hairline. `shadow-craft-md/lg/xl` still exist for the
dashboard; do not use them on marketing surfaces.

---

## Surfaces

| Class | Use |
| --- | --- |
| `.surface` | Cards. White, 1px hairline, 12px. |
| `.surface-lg` | Feature and showcase panels. White, 1px hairline, 16px. |
| `.surface-alt` | Nested tonal panel. Paper, no border, 16px. |
| `.pill` | Tags, badges, eyebrows. White, 1px hairline, 9999px. |
| `.mockup-frame` | Product screenshots. White, hairline, 16px, 4px ring. |

`glass-card` / `glass-chip` / `glass-strong` are **legacy**. They still exist for
`/dashboard`, `/admin` and `/share`, and are neutralised to border-first under
`[data-site="marketing"]`. Do not author new marketing markup with them.

### Nested surfaces and the @theme alias trap

Tailwind v4 `@theme` aliases are declared once, on `:root`:

```css
@theme { --color-canvas: var(--canvas); }
```

The inner `var()` resolves **against `:root`**, not against the element using the
utility. So a token overridden on a *nested* element can never reach an aliased
utility. `<div data-theme="dark" class="bg-canvas">` mid-page stays light, and it
fails silently — nothing errors, the colour just never changes.

This is why the closing CTA does **not** use `bg-canvas` / `text-ink`. It uses
`.band-dark`, which sets literal oklch values and its own local vars:

| Class | Use |
| --- | --- |
| `.band-dark` | The always-dark surface. Sets `--band-ink`, `--band-muted`, `--band-line`. |
| `.band-muted` | Secondary text on the band. |
| `.band-rule` | Hairline as a **border** (buttons, `border-y`). |
| `.band-gap` | Hairline as a **background** (a `gap-px` grid's separators). |
| `.band-surface` | Restores the band's own fill on a cell inside a `gap-px` grid. |

`band-rule` and `band-gap` are split on purpose: one class setting both would
give an outlined button a filled background.

The rule: **a surface that flips colour mid-page must carry literal values.**
Aliased tokens only work when the override lands on `:root`, which for this site
means `<html data-site="marketing">`.

### Backgrounds

The canvas is white. The only textures are:

- A faint 56px line grid, applied once globally in the root layout with a radial
  fade. This is the blueprint signature.
- `bg-paper` bands for tonal section separation.

`bg-aurora`, `bg-ambient`, and the full-bleed photo backdrops (`HeroBackdrop`,
now deleted) are gone. They carried real contrast cost: the hero photo forced a
five-stop gradient wash just to keep the headline readable.

---

## Layout

### Column rules and bleed

Two hairlines run the full viewport height at the content column's edges
(`+layout.svelte`), so the page reads as one ruled sheet. What crosses them is
a hard rule:

| Section has | Width |
| --- | --- |
| A tonal background (`bg-paper`) | **Full-bleed** to the viewport |
| No background | **Bounded** — `mx-auto max-w-6xl` so it sits inside the rules |

The closing CTA's dark band is full-bleed. Mixing the two is what makes the
rhythm read; a page where everything bleeds loses the rules entirely.

- **Container:** `<Container>` — `max-w-6xl` default, `narrow` (3xl), `wide` (7xl).
- **Section:** `<Section spacing="default | tight | loose | none">` — default is
  `py-16 md:py-24`. Roughly 64–96px, down from the previous 96–128px.
- Section dividers: `border-t border-border-low`. No solid rules, no gradients.
- Pages end with `<Footer />`.

### Page rhythm

1. **Hero** — split announcement pill, two-line headline with the rotating word
   on its own line, one-paragraph subhead, filled + outlined CTA pair, meta line.
2. **Step shelf** — full-bleed paper band; the white hero bulges down into it
   through two concave fillets, carrying the Record / Polish / Share tabs and the
   product mockup.
3. **Proof** — paper band, before/after comparison.
4. **Trust strip** — values row plus the open-source stack logos.
5. **Spine sections** — Record, Polish, Share, each a `<ShowcasePanel>`.
6. **Supporting beats** — the editor marquee, extensions, Cloud (chapter `04`,
   same rhythm as the pillars), founders, pricing teaser.
7. **FAQ** — sticky title left, single-open accordion right.
8. **Closing CTA** — bookends the hero. The same notched shelf bridges into the
   page's one dark band, and the three-step spine is restated as a one-line
   recap in the same three hues, so the page ends where it began.
9. **Footer**.

---

## Components

### Hero step shelf

[HeroSteps.svelte](src/lib/components/HeroSteps.svelte). A `tablist` of the three
spine steps that hangs out of the white hero into the paper band below.

- Geometry: shelf `rounded-b-[32px]`, two 32px masked fillets. The fillet radius
  **must** track the shelf's bottom radius or the joint reads as two shapes.
- Tabs take their sizing and focus treatment from `buttonVariants({ variant:
  "ghost" })` so they cannot drift from the button system.
- Selected state cross-fades a white skin in underneath the label. Unselected is
  bare — no resting fill, no border.
- Auto-advances every 5.2s; pauses on hover and focus; pinned under
  `prefers-reduced-motion`.
- Only the visible clip decodes. Three concurrent `<video>` decodes is the
  cheapest way to make a landing page stutter.
- Per-step `src` falls back to a shared take, so the cross-fade is wired before
  the three clips exist.

### PillarSection

[PillarSection.svelte](src/lib/components/PillarSection.svelte) renders Record /
Polish / Share as **numbered chapters**. The page's spine is three ordered
steps, so each pillar opens on a chapter rule: the index numeral in the display
face, the product label, and the section's one action, all on a single hairline
across the column.

Below it, a 5/6 split: headline and description on the left, the step's details
stacked down the right, divided by hairlines. A two-column spread reads
editorial; a centred stack reads like a slide.

Colour appears exactly once per section, on the duotone detail glyphs. There is
**no accent rule and no lit/unlit state** — three details of equal weight are
three facts, not a control that needs a selection. (The earlier version lit one
column with a coloured left border on hover; it read as a selected tab that
wasn't selectable, and dimming the unlit copy failed 4.5:1.)

The visual sits in a full-bleed `bg-paper` band below.

Scroll-in uses `<Reveal>`. **Never hand-roll an IntersectionObserver here** —
Reveal falls back to visible when the observer is missing, resets itself under
`prefers-reduced-motion`, and carries the one shared easing curve. A local copy
leaves the whole section stuck at `opacity-0` when JS never runs.

### FeatureMarquee

[FeatureMarquee.svelte](src/lib/components/FeatureMarquee.svelte) — the editor
tour as one slow horizontal loop (64s) rather than a rail the visitor has to
drag. Ambient, not a carousel: no arrows, no dots.

- The track holds the list **twice** and translates exactly `-50%`, so the seam
  lands on an identical frame and the loop is invisible. The second copy is
  `aria-hidden` — a screen reader should hear the tour once.
- Both edges cross-fade via `mask-image`, so cards dissolve into the page
  instead of being sliced by the container edge.
- Hover or focus-within parks the animation, so a card can actually be read.
- **Card art is vector only.** Screenshots in a 64s loop are a lot of bytes for
  something that scrolls past, and they age with every UI change. Each card is a
  duotone glyph on `bg-paper` over two SVG rings that ping outward, staggered per
  card. `editorFeatures` no longer carries an `image` field.

**Reduced motion needs an explicit kill here.** The global guard collapses
`animation-duration` to `0.01ms`, which would snap the track straight to its end
frame. The component's own `@media (prefers-reduced-motion: reduce)` sets
`animation: none` on every loop and turns the rail into an ordinary scroller.

Badges are `Auto` / `Manual` at `text-caption` with a 4px dot — not the old
mono, uppercase, letter-spaced `AUTOMATIC`.

### Product mocks

`RecordMock`, `PolishMock` and `ExportMock` are one set, one per pillar. They
are vector and CSS only; no new animation dependency. `gsap` is already in the
app for `TextLoop`, and Svelte's own transitions cover the rest.

Rules that keep them consistent with the page rather than with each other:

- **Framed by `MacWindow`, never by a card.** All three pillar visuals are the
  same window chrome. A mock inside a `.surface-lg` that already has its own
  heading is a card in a card, and it reads as a different design system.
- **The mock carries no heading.** The pillar already states the claim on the
  left column; repeating it inside the visual is duplicated copy and bulk.
- **Hairlines, not tiles.** Status stages are `gap-px` cells over
  `bg-border-low`, the same grid the extensions row uses. No rounded step
  circles, no per-stage progress bars — one track for the whole transfer.
- **One timeline drives everything.** A single `elapsed` counter derives every
  phase so the parts can never disagree about which state they are in.
- **Move one object, don't cross-fade many.** `RecordMock` drags one marquee;
  `PolishMock` lands one edit at a time; `ExportMock` fills one rail.
- **Inert controls.** A mock's "Copy link" uses `buttonVariants` for exact
  parity with the real button but renders as an `aria-hidden` span: a mock
  control must not take focus.
- **Scrims darken in both themes.** A `foreground`-tinted scrim *brightens* the
  excluded area in dark mode; dimming is always black at alpha.

Every loop needs a `prefers-reduced-motion` branch that pins the mock to a
finished state — selection made, all edits applied, link present.

### FaqList

One FAQ component for every page (`/`, `/pricing`). It is built on
`@recast/ui/collapsible`, not `<details>`: the shared Collapsible animates real
height through Svelte's `slide`, which native `<details>` cannot do at all.
One row open at a time, first row open on load, plus/rotate as the only
affordance. No card, no chevron column, just hairline-divided rows.

Anything with a code block (the install troubleshooting on `/download`) uses
`Collapsible` directly instead, since FaqList takes plain `{ q, a }` text.

### IntegrationGrid

The share-destination field on `/features`. A `gap-px` grid where **the empty
cells are load-bearing**: a full grid reads as a finished list, a sparse one
reads as a set still filling up. Slot indices are fixed, never random, so the
scatter is stable across renders and hydration.

Marks sit directly in the cells with no coloured tile behind them. A shipped
integration renders at full ink with a `tag-green` dot; everything else sits at
`border-strong` and lifts to full ink on hover. That difference is the honesty
in the section: today only Drive is live, and the grid says so rather than
implying ten working integrations. Each cell carries an `sr-only` label with
its status, since the mark alone is not an accessible name.

Third-party marks come from `@recast/icons` as `Brand*` aliases. Add new ones to
the Tabler barrel rather than importing `@tabler/icons-svelte` directly at a
call site.

### Spotlight rows

Two-up hairline row of product mocks, each with a heading, one paragraph and an
outline action, sitting above a denser 4-up grid of everything else. Use it when
two features deserve a moving visual and the rest deserve a line. The mocks are
the same `PolishMock` / `ExportMock` the home page uses, so the pages agree
about what the product looks like.

Never add a "Learn more" affordance to the dense grid unless every row has a
real destination. The old catalog rendered one on all 17 cards, all pointing at
`#`.

### Two-pane index (changelog)

A rail of entries beside the entry body. Rules that make it read as one object
rather than a list next to a card:

- **The rail is flush.** Rows run edge to edge inside their pane, divided by
  hairlines, and selection is a `bg-paper` fill plus a marker growing from the
  column rule. Rows inset with their own padding, radius and border read as
  cards floating in a sidebar, which is a different design system.
- The two panes are one `gap-px` grid, so the divider between them is the same
  hairline as everything else.
- Motion carries the selection: the marker scales from the rule, the arrow
  slides in, and the body re-enters on `{#key}` with a short `fly`. Every one
  has a `motion-reduce` branch.
- Selection is addressable. The hash follows the selected entry and is read back
  on load, so a release can be linked directly.
- Arrow keys move the selection and move focus with it. A dense list should not
  need the mouse.

### Error pages

`+error.svelte` is a marketing surface, so it follows the interior-hero shape:
status numeral on the chapter rule, display heading, one line of body, then
actions. No radial backdrop, no grid wash, no icon tile.

Next steps stay at three, as a hairline grid. A full site map on an error page
reads like a dead end. Dev-only detail sits behind a `<details>` on a hairline,
never a bordered card.

`ACCENT_TEXT` maps status to flat ink for this page; `ACCENT_RING` and
`ACCENT_BACKDROP` stay for the in-shell `SectionError`, which still lives on the
product system.

### Auth pages

A split, not a floating card. `(auth)/+layout.svelte` is a 12-column grid: the
form pane (7) carries a hairline header (mark, back link) and a hairline footer
(legal, download), and the brand pane (5) sits on `bg-paper` with the
Record/Polish/Share spine read straight from `Hero.logic`. Auth is the first
screen many people see; it should say what the product is.

The brand pane is `hidden lg:flex`, not a stacked block. On a phone the form is
the only thing worth the viewport.

`AuthCard` is a heading block, not a card: eyebrow, display `h1`, one line of
body, then the form. The pane is already the surface, so a bordered card inside
it is the card-in-card mistake again, and the layout header owns the logo. Every
route in `(auth)` uses it, including the three that used to hand-roll their own
header and card (`accept-invitation`, `device`, `verify-email`).

Everything is left-aligned, like every other page. Fields are hairline
(`border-border-low bg-background`) rather than the filled `bg-input` default,
and the primary action is `variant="dark"`, so an auth screen and a marketing
page agree about what a form and a button look like.

Status moments are a glyph, not a plate. The device-approved state was a
`size-16` tinted tile with a `ping` ring; it is now the check itself at
`size-10` in `tag-green`. Inline warnings are a hairline block on `bg-paper`,
because a tinted amber card is a second colour system for one sentence.

### Dashboard shell

The product routes keep their own tokens (the dashboard is not under
`[data-site="marketing"]`) and their own shape: `Sidebar.Root` stays
`variant="inset"`, because the floating panel is the shell's identity. What
changed is everything the token system covers:

- **No glass.** The header was `bg-background/80` + `backdrop-blur-xl`; it is
  opaque on a full-strength `border-border-low` rule. Same for the search
  trigger, which was a `bg-card/70 backdrop-blur` inset card and is now a
  hairline field.
- **No dilutions.** `border-border-low/60`, `/70`, `border-border/30`,
  `text-muted-foreground/40` and `bg-foreground/5` all resolve to the real
  tokens (`border-border-low`, `bg-paper`).
- **Type on the scale.** `text-[15px]`, `text-[12.5px]`, `text-[12px]`,
  `text-[11px]`, `text-[10px]`, `text-[9px]` are gone; group labels are sentence
  case rather than uppercase and tracked.
- **One filled button per surface.** "New Recast" is `variant="dark"` like every
  other primary; the GitHub nudge in the footer drops to `outline`.

**The search field carries a fill.** `bg-paper` against the header's
`bg-background`, because an outline alone on the same colour as its surroundings
does not read as a field. Below `sm` it collapses to its icon at `size-9`: a
full-width bar costs more than a phone header has to spend.

**Header search stays centred** in its `mx-auto max-w-md` box, with the empty
spacer kept on `/dashboard` (where the hero owns search) so the profile menu
holds its right-edge anchor.

The same sidebar component backs the admin shell through its flat `nav` prop, so
both shells moved together.

### Dashboard home

Same token pass as the shell, and deliberately **no change to what is on the
page or where**: greeting, hero search, Upload/Analytics actions, the metric
strip, library counts, activity, usage and recent recasts all stay in place and
in order. A dashboard people already know is not the place to move the furniture.

- `glass-card` → `.surface`, `glass-chip` icon tiles → the glyph on its own.
  A 40px tinted plate behind a 16px icon is the thing the marketing pages
  dropped first; the dashboard had one on every card header, empty state and
  activity row.
- The greeting was `bg-clip-text` over a `to-foreground/60` gradient, which
  fades a page's one `h1` into the background. Solid display type now.
- The metric strip sits between two hairlines instead of floating, so it reads
  as one object rather than five stray numbers.
- Labels drop uppercase tracking for sentence-case `text-caption`; every
  `text-[9px]`…`text-[11px]` goes to the scale; `font-mono` readouts become
  `tabular-nums`.
- Upload is `variant="dark"` (the page's conversion action), Analytics stays
  outline: one filled button per surface.
- Dilutions resolve to tokens: `divide-border-low/40`, `ring-border-low/40`,
  `bg-foreground/5`, `bg-foreground/8` → `border-border-low`, `bg-paper`. The
  one surviving alpha is the play-overlay scrim on a poster, where dimming an
  image is the actual job.

**Cards keep both the fill and the hairline here, and that is not redundant.**
The product theme lifts `--card` above the panel, but the lift measures 1.5 L in
light (imperceptible) against a 9 L border, and 6.65 L against a 5 L border in
dark. Light mode is carried by the hairline, dark by both. Drop either and one
theme loses the card edge. That is the difference from the marketing surface,
where `card == canvas` and the hairline is the only differentiator.

**An empty workspace gets one instruction, not five empty panels.** With no
recasts, usage, activity and rankings all read zero and bury the only action
that matters, so the page collapses to a single card: what fills in, three
steps, and the two buttons. The hero stays, because search and Upload are still
the point.

### Team page

Same pass, and the shared pieces moved with it: `PageHeader` (a `size-11`
`glass-chip` plate behind a `size-5` glyph) and `SettingsSection` (`glass-card`
plus `ring-1 ring-foreground/12` for its accent). Both are used across settings,
team and library, so those routes inherit the fix.

**The accent variant was a ring on a card.** A ring outside a border is two
outlines on one box; emphasis is now `border-border-strong` on the hairline the
card already has.

Content changes, since the page was dense but not evenly useful:

- **"Your role" left the stat row.** Your own access level is identity, not a
  metric, and it is already legible from the rows and from which controls you
  can see. It moved to a badge in the page header, where identity belongs.
- **"Can manage" took its place** — how many owners and admins the workspace
  has, which is the number worth watching on a shared account.
- **A seat-capacity bar** sits under the stats when the plan caps members. It
  states the allowance before someone writes an invite they cannot send, and
  when every seat is taken an owner gets "Add seats" inline. Pending invites do
  not hold a seat, and the copy says so.

### Interior pages

Both were on the pre-Dub system: floating `rounded-2xl` cards, `blur-3xl` glow
blobs, hardcoded `amber-500`/`emerald-500` state colours, mono labels and
ad-hoc `text-5xl`/`text-sm` sizes. Rebuilt on the marketing system:

- Plans are **one `gap-px` hairline grid**, not three floating cards. The
  featured plan is marked by a `bg-paper` header block and a badge, never by a
  coloured ring or a glow.
- Every heading uses the display face; every number uses `tabular-nums`.
- State colour comes from the tag hues (`tag-green` stable, `tag-tangerine`
  caution). No palette colours; they had no dark-mode pairing.
- Long tables stay hairline grids: `border-y` header, `bg-paper` group rows,
  `Check` for yes and a neutral dot for no. A muted dot reads as "not included"
  without the visual weight of a minus glyph in every empty cell.
- Segmented controls (billing period, platform tabs) are one shape: a
  `border-border-low` + `bg-paper` track with the active pill on `bg-background`
  and `shadow-craft-sm`.

**Every interior hero is the same shape**, and it is not the home page's hero:
`SectionLabel`, display `h1` at `text-heading-lg md:text-display`, one
`text-body-lg` line, then actions. Left-aligned in the `max-w-6xl` column, with
a hairline rule under it carrying the page's one-line facts (guarantees on
`/pricing`, build stability on `/download`, last-updated on the legal pages).
Centred hero stacks with italic second lines and `lg:text-[5rem]` are gone.

`/extensions` follows the home page's section grammar exactly: chapter rule,
heading column, then a `gap-px` hairline grid. Its install steps use the same
display-face `01` numerals as the home page pillars rather than icon tiles.

Legal pages keep their `max-w-3xl` measure but sit **inside** the column rules
rather than centred in the viewport, so the rules line up with every other page.

### Footer wordmark

The oversized "Recast" at the foot of the page is filled with a
`foreground`-to-transparent gradient clipped to the text, with a brighter band
that drifts across it on a 9s loop. Colour comes from `color-mix` on
`--color-foreground`, so it inverts with the theme and stays token-driven.

Reduced motion needs an explicit `animation: none` plus a flat gradient. The
global guard only collapses duration to `0.01ms`, which parks the sheen
mid-sweep instead of removing it.

### SectionLabel

Duotone glyph in the section's accent hue plus a plain label. No pill, no
tinted tile behind the icon, no uppercase tracking. Duotone is
`fill="currentColor"` plus `[fill-opacity:0.2]`.

### NotchedShelf

[NotchedShelf.svelte](src/lib/components/NotchedShelf.svelte) — the shape that
bridges two surfaces, used by the hero step shelf and the closing CTA.

Two mirrored S-curve SVG wings (85×64) with a `grow` bar between them. An
S-curve, not a circular fillet: a quarter-circle meets the straight edge at a
visible corner, the S-curve eases out of it.

Two traps, both hit once already:

- `fill` is a **Tailwind text-colour class** (`text-background`), not a CSS
  value. The wings and bar are all `currentColor`. Passing a raw `var(...)`
  lands as an invalid class and the shelf paints nothing.
- Never put a text colour on the bar itself — it is `bg-current`, so
  `text-foreground` there repaints the bar near-black. The content's colour
  reset belongs on a nested element.

### Navbar

Minimal top bar: logo left, links centred, ghost sign-in + filled dark Download
right. **Transparent with no border until scrolled** past 8px, then hairline +
canvas. A permanent border reads as a frame around the page.

### Buttons

- Primary: `<Button variant="dark">` — near-black fill, white text. One per view.
- On `.band-dark`: primary is `variant="light"`, secondary is `variant="outline"`
  plus `band-rule` and `text-current`. Platform rows are Buttons too, not bare
  anchors, so they inherit the system's sizing and focus ring.
- OS marks come from `@recast/ui/brand-icons` (`WindowsBrand`, `AppleBrand`,
  `LinuxBrand`) so the home page and `/download` show the same glyphs.
- Secondary: `<Button variant="outline">` — white, hairline, border darkens on hover.
- Press feedback is `active:scale-[0.99]`. There is no hover-grow and no
  radius-morph-on-press; both were removed from `@recast/ui` in this pass.

### TextLoop (rotating word) — brand identity, do not change

```svelte
<h1>
  Record once.
  <span class="mt-1 flex justify-center italic text-muted-foreground">
    <span class="whitespace-nowrap">Ship a&nbsp;</span>
    <SelectionWord>
      <span class="inline-grid overflow-hidden">
        <TextLoop class="text-primary" texts={words} interval={3000} />
      </span>
    </SelectionWord>
  </span>
</h1>
```

The rotating word **must** sit on its own block-level line; GSAP animates the
inner width and the outer line stays independent so the headline never reflows.
The italic here is deliberate and is the one place italic is allowed — it frames
the rotating output as an editable object. Section h2s do **not** get it.

### Reveal

Wrap scroll-in content in `<Reveal delay={i * 70}>`. Stagger lists 60–80ms.

### Motion

One ease: `cubic-bezier(0.625, 0.05, 0, 1)`, exported as `CRAFT_EASE`.

| Use | Duration |
| --- | --- |
| Hover / state colour change | 200ms |
| Cross-fade (shelf skin, clip swap) | 300–420ms |
| Overlay enter / exit | 200 / 150ms |
| Sheet enter | 300ms |

Scale deltas are 2% (`0.98`), never smaller. Svelte `transition:` directives use
WAAPI and **bypass the CSS reduced-motion guard** — gate them in JS with
`prefersReducedMotion()`.

### Trust strip

Recast is in beta. **Do not fabricate customer logos.** The open-source stack is
the honest social proof, rendered in a muted neutral via Simple Icons.

---

## Dos and Don'ts

**Do**

- Define containers with `border-border-low` at full strength.
- Use the nine-step type scale and the five-radius vocabulary.
- Keep Satoshi for h1/h2 only; Inter handles everything at 30px and below.
- Give each feature tag exactly one hue, rendered duotone.
- Reserve `--primary` to the listed roles; commit actions are near-black.
- Test new sections in both light and dark.

**Don't**

- Don't fade text with an opacity modifier — use `text-muted-foreground`.
- Don't dilute the hairline (`border-border-low/40` and friends).
- Don't add `backdrop-filter` or `shadow-craft-md`+ to a marketing surface.
- Don't write `text-[13px]` or any other ad-hoc size.
- Don't use `uppercase` + letter-spaced eyebrows; use the sentence-case pill.
- Don't put a photo behind a headline.
- Don't italicise section headings.
- Don't invert the theme mid-page. The one exception is the closing CTA band,
  which is `data-theme="dark"` by design; its notched shelf must stay outside
  that subtree or `--color-background` resolves dark and the bridge vanishes.

---

## Routes & section anchors

| Route | Sections |
| --- | --- |
| `/` | `#proof`, `#why`, `#record`, `#polish`, `#editor`, `#extensions`, `#share`, `#cloud`, `#founders`, `#pricing-teaser`, `#faq`, `#cta` |
| `/pricing` | hero, plan cards, comparison table |
| `/features` | pillars, supports, `#cta` |
| `/download` | hero, `#all-platforms` |
| `/changelog` | hero, release timeline |
| `/architecture` | hero, system map, one section per domain |
| `/architecture/[slug]` | header, facts panel, invariants, prose, pager |

Keep navbar and footer links in sync. Stale anchors are silent UX bugs.

---

## Architecture pages

`/architecture` is a reference surface, not a marketing one, and it borrows the
marketing system rather than inventing a second. Three rules keep it honest:

- **Facts before prose.** Every page opens with a four-panel band, what goes in,
  what comes out, where to start reading, and the invariants, built from
  frontmatter. A reader who stops there has still learned something.
- **The system map is border-first like everything else.** Its nodes are a
  hairline, a radius, and a 3px left edge in the phase's hue. Record is
  tangerine, Polish lavender, Share green, and an artifact is neutral, so the map
  reads as the same spine the landing page tells.
- **Prose runs wider here** (`78ch`, `width="reference"`) than in an article
  (`68ch`). Reference tables and diagrams need the measure; an essay does not.

The map is `aria-hidden` and paired with a disclosure that lists the same graph
as links. A node-graph is not navigable by keyboard or screen reader, so the
list is the real content and the picture is the illustration.
