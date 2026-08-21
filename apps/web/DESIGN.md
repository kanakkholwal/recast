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
| Card | `oklch(100% 0 0)` | `oklch(17.5% 0 0)` | `--card` |
| Paper (tonal band) | `oklch(97% 0 0)` | `oklch(20.5% 0 0)` | `--paper` → `bg-paper` |
| Hairline | `oklch(92.2% 0 0)` | `oklch(26.9% 0 0)` | `--border` → `border-border-low` |
| Emphasis border | `oklch(87% 0 0)` | `oklch(33% 0 0)` | `--border-emphasis` → `border-border-strong` |
| Text | `oklch(20.5% 0 0)` | `oklch(96% 0 0)` | `--foreground` |
| Muted text | `oklch(55.6% 0 0)` | `oklch(64% 0 0)` | `--muted-foreground` |

Light-mode canvas is pure white and cards are pure white. They are told apart by
the hairline, not by tone. That is the whole idea.

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

- **Display (h1, h2):** Satoshi, weight **500**, letter-spacing **normal**.
- **Everything else:** Inter — body, UI labels, h3–h6 (weight 600, `-0.011em`).
- **Mono:** Geist Mono, for code, file names, and stat numbers.

Weight 500 is the signature. Headings are confident because of the letterforms,
not because they are bolded or tracked tight. `font-bold` on a heading is a bug.

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
| `text-heading-lg` | 36px | 1.11 |
| `text-display` | 48px | 1.04 |
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

The one exception is the hero shelf's 32px concave fillet, which is a *shape
constant* (it must equal the shelf's own bottom radius) rather than a component
radius. It is documented in [HeroSteps.svelte](src/lib/components/HeroSteps.svelte).

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
6. **Supporting beats** — extensions, Cloud, founders, pricing teaser.
7. **FAQ** — sticky title left, single-open accordion right.
8. **Final CTA**, then **Footer**.

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

### Navbar

Minimal top bar: logo left, links centred, ghost sign-in + filled dark Download
right. **Transparent with no border until scrolled** past 8px, then hairline +
canvas. A permanent border reads as a frame around the page.

### Buttons

- Primary: `<Button variant="dark">` — near-black fill, white text. One per view.
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
- Don't invert the theme mid-page. The site is one surface.

---

## Routes & section anchors

| Route | Sections |
| --- | --- |
| `/` | `#proof`, `#why`, `#record`, `#polish`, `#editor`, `#extensions`, `#share`, `#cloud`, `#founders`, `#pricing-teaser`, `#faq`, `#cta` |
| `/pricing` | hero, plan cards, comparison table |
| `/features` | pillars, supports, `#cta` |
| `/download` | hero, `#all-platforms` |
| `/changelog` | hero, release timeline |

Keep navbar and footer links in sync. Stale anchors are silent UX bugs.
