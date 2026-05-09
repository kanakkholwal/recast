# Recast Web — Design System

The marketing site for Recast. This document captures the design language, voice,
and component patterns so every page lands consistently.

> **Stay opinionated.** Recast is a tool for people who'd rather ship than fiddle.
> The site should feel the same way: confident, clean, and never overwrought.

---

## Audience

Primary readers, in order of priority:

1. **Solo founders** — building, demoing, and pitching weekly.
2. **Indie hackers** — shipping launch videos, changelog clips, and Twitter cuts on their own schedule.
3. **Product engineers** — demoing PRs, explaining bugs, documenting APIs.

Every headline, microcopy line, and CTA should serve at least one of these readers.
If a sentence reads as if it's targeting an enterprise procurement team, rewrite it.

---

## Voice

| Do | Don't |
| --- | --- |
| Direct, opinionated, founder-to-founder. | Marketing-speak ("solutions", "synergy", "leverage"). |
| Concrete verbs ("ship", "record", "trim"). | Vague abstractions ("empower", "transform"). |
| Punchy, balanced two-line headlines. | Long paragraph hero copy. |
| Emphasize *outcomes* the audience cares about (looking expensive, shipping fast). | Generic feature lists without a clear "so what?". |
| Use "Skip the editor. Ship the demo." as the through-line. | Compete on feature checklists with kitchen-sink editors. |

**Headline pattern:** *[Aspiration] that look [outcome word].*

- Demos that look **expensive.**
- Demos that look **cinematic.**
- Demos that look **intentional.**
- Demos that look **effortless.**
- Demos that look **hand-edited.**

The rotating word is always italic, lower opacity, primary-tinted, and lives on its
**own line** (see "TextLoop" below) so the headline never reflows.

---

## Color & Theme

Tokens live in [`@recast/design`](../../packages/design/src/index.css). Always use
CSS variables — never hardcode colors.

| Token | Use |
| --- | --- |
| `--background` / `--foreground` | Page background and primary text. |
| `--card` / `--card-foreground` | Surface containers (glass cards, mockups). |
| `--primary` (vivid lime green) | Accents, CTA buttons, callouts, and brand emphasis. |
| `--muted-foreground` | Secondary copy, microlabels. |
| `--border` / `--border-low` / `--border-strong` | All separators, subtle dividers. |
| `--destructive` / `--success` / `--warning` | Status only — never decorative. |

### Dark mode

The dark-mode `--primary` is a **highly saturated lime** (`oklch(92% 0.234 …)`).
Even small blends create heavy contrast. When mixing primary into backgrounds,
**stay below 8%** in dark mode (vs ~15% in light). When in doubt, use neutral
foreground tints (`color-mix(... var(--color-foreground) 4-6%)`) for ambient glows
instead of primary.

> Lesson learned: a `bg-aurora` with primary ~22% in dark mode produces a heavy
> green band behind the navbar that looks oversaturated. Tone it down.

---

## Typography

- **Sans / display:** `Geist Variable`. Tight tracking (`-0.02em`), `font-feature-settings: "ss01", "cv11"`.
- **Mono:** `Geist Mono Variable`. Use for code blocks, file names, stat numbers.

### Scale

| Use | Size |
| --- | --- |
| Hero h1 | `text-5xl … lg:text-[5.25rem]`, `font-semibold`, `leading-[1.02]` |
| Section h2 | `text-3xl … md:text-5xl`, `font-semibold`, `tracking-tight` |
| Card h3 | `text-lg … text-xl`, `font-semibold`, `tracking-tight` |
| Body | `text-base … sm:text-lg`, `leading-relaxed`, `text-muted-foreground` |
| Eyebrow | `text-[11px]`, `font-semibold`, `uppercase`, `tracking-[0.18em]`, `text-muted-foreground` |
| Microlabel | `text-[10px]`, `font-bold`, `uppercase`, `tracking-[0.16em]` |

Use `text-balance` on every headline. Use `text-pretty` on every body paragraph.

---

## Layout

- **Container:** `<Container>` — `max-w-6xl` default, `narrow` (3xl), `wide` (7xl), `full`.
- **Section:** `<Section spacing="default | tight | loose | none">` — `py-24 md:py-32` default.
- Pages always end with `<Footer />`.
- Section dividers: `border-t border-border-low/60`. No solid horizontal rules.

### Page rhythm

A marketing page composes top to bottom in roughly this order:

1. **Hero** — atmospheric background, eyebrow chip, two-line headline (with rotating word on its own line), one-paragraph subhead targeting the audience, primary + outline CTAs, beta line, then a glass-card preview screenshot with floating chips.
2. **Trust strip** — short uppercase eyebrow ("Built on tools makers trust") plus the tech-stack logo row (see "Trust strip" below). Honest credibility for a beta product — never fake customer logos.
3. **Triad cards** — three audience-targeted cards, each with a mini visual.
4. **Conversion section** — two columns: checklist of outcomes + a product preview screenshot with floating chips.
5. **Dual feature cards** — side-by-side mockup cards (e.g. command palette + activity bars).
6. **Big showcase** — single hero screenshot with a wash of primary glow behind it.
7. **Format / code section** — two columns: bullet list + code/config snippet styled like a tabbed editor.
8. **Intelligent features** — triptych mockup row, then a 4-cell feature row.
9. **Final CTA** — full-width glass card with eyebrow chip, hero-scale headline (with italic emphasis on second line), description, dual CTAs.
10. **Footer**.

Not every page needs every section, but ordering and visual rhythm must match.

---

## Components

### Glass surfaces

| Class | Use |
| --- | --- |
| `glass-card` | Feature cards, mockup containers, CTA cards. |
| `glass-chip` | Floating labels, status pills, icon containers. |
| `glass` / `glass-strong` | Navbar-style overlays. |

Glass surfaces always sit on top of an atmospheric or grid background — never
flat. Pair with `shadow-craft-md`, `shadow-craft-lg`, or `shadow-craft-xl` for
depth, plus `rounded-2xl` (cards) or `rounded-[2rem]` (CTA hero card).

### Eyebrows

```svelte
<Eyebrow icon={Sparkles} variant="primary">v0.1 beta · what's new</Eyebrow>
```

Always lead a section with an eyebrow chip when the section has a header.

### Section header

```svelte
<SectionHeader
  eyebrow="Built for modern makers"
  title="A recorder shaped to your workflow."
  description="…"
  align="left | center"
/>
```

### Buttons

- Primary CTA: `<Button size="lg" class="gap-2.5">` with a leading icon.
- Secondary CTA: `<Button variant="outline" size="lg">` with a trailing arrow.
- **Avoid** `variant="ghost"` next to a solid button — visual weight is too uneven.
- Trailing arrows use `transition-transform group-hover/cta:translate-x-0.5`.

### Reveal

Wrap any content that should fade-up on scroll in `<Reveal delay={i * 60}>`.
Stagger lists by ~60–80ms per item. Use `as="li"` when wrapping list items.

### Motion vocabulary

Recast has **one motion ease**: `cubic-bezier(0.625, 0.05, 0, 1)`, exported as
`CRAFT_EASE` from `@recast/ui/utils`. It's snappy at the start and lands gently —
the same curve the FloatingMenu uses for its open/close timeline.

| Use | Duration | Notes |
| --- | --- | --- |
| Hover state changes | `duration-200` | bg/text/opacity shifts on buttons, links, chips |
| Overlay enter (dropdown, popover, tooltip, hover-card, dialog) | `duration-200` | via `CRAFT_OVERLAY_ANIMATION` |
| Overlay exit | `duration-150` | slightly faster than enter — feels responsive |
| Sheet enter | `duration-300` | longer feels intentional for full-edge surfaces |
| Backdrop fade | `duration-200` enter / `150` exit | via `CRAFT_OVERLAY_BACKDROP_ANIMATION` |

**Subtlety rules:**
- Scale: `0.98` (2% delta) — never `0.95` or smaller; that reads as "popping".
- Slide: 4px (`slide-in-from-X-1`) for popovers, 24px (`slide-in-from-X-6`) for sheets.
- Avoid `ease-in-out` and `ease-linear` for state transitions — use `CRAFT_EASE`.

**For new bits-ui Content components**, import `CRAFT_OVERLAY_ANIMATION` from
`@recast/ui/utils` and prepend it to the class list. Don't hand-roll `data-open:`
animation classes per component — they will drift.

```svelte
<script>
  import { CRAFT_OVERLAY_ANIMATION, cn } from "@recast/ui/utils";
</script>

<Primitive.Content class={cn(CRAFT_OVERLAY_ANIMATION, "rounded-lg ...", className)} />
```

**Svelte transitions** (`fly`, `fade`, `slide` from `svelte/transition`) are
preferred for any in-app component you control directly. Use bits-ui's
data-state CSS animations only for portal-mounted overlays where Svelte's
`transition:` directives can't reach.

### TextLoop (rotating word)

Layout-shift safe pattern:

```svelte
<h1>
  Demos that look
  <span class="mt-2 flex justify-center font-medium italic text-foreground/40">
    <span class="inline-grid overflow-hidden">
      <TextLoop class="text-primary" texts={words} interval={3000} />
    </span>
  </span>
</h1>
```

The rotating word **must** sit on its own block-level line (a `flex` row works);
GSAP animates the inner width but the outer line is independent so the rest of
the headline never reflows.

### Trust strip

Recast is in beta — **don't fabricate customer logos**. Use the open-source tech
stack as honest social proof. Logos render via [Simple Icons CDN](https://simpleicons.org)
in a muted neutral (`9ca3af`) tone so they read as a strip, not a competing focal point.

Each entry links to the project, has hover opacity transition, and shows a
text label next to the icon for readers who don't recognize the mark.

If you want to show real users in the future, switch the heading to
"Loved by teams at" and use **only** companies that have explicitly opted in.

### Atmospheric backgrounds

Never use solid color section backgrounds. Use one of:

- `bg-aurora` — radial primary glows, top-weighted.
- `bg-ambient` — softer, multi-corner radial wash.
- `bg-grid bg-grid-fade` — subtle 56px grid with radial mask.
- `bg-dots` — 20px dot pattern.

Hero sections layer: aurora + clouds + grid (in that z-order) for depth.

---

## CTA pattern

The "Skip the editor. Ship the demo." CTA is the canonical end-of-page card.
Reuse this exact structure on every page that needs a closing CTA:

- Pulsing beta chip ("v0.1 beta · ready when you are")
- Hero-scale h2 with italic emphasis on the second line
- One-line supporting copy ("Free during beta. No account required. Three platforms. One opinionated tool.")
- Solid primary CTA (`Download Recast`) + outline CTA (`See what's new` / `Explore features`)
- Top-positioned radial primary glow (~22% opacity) + 1px hairline gradient on top edge

---

## Dos and Don'ts

**Do**

- Use markdown link syntax (`[file.svelte](src/lib/components/file.svelte)`) when referencing code.
- Use [Lucide icons](https://lucide.dev) only.
- Reference design tokens via Tailwind utilities (`bg-primary`, `text-muted-foreground`).
- Reach for `glass-card` + `shadow-craft-*` for elevated surfaces.
- Test new sections in **both** light and dark — primary saturation differs dramatically.

**Don't**

- Hardcode hex/rgb colors. Use CSS variables and `color-mix()` in `srgb`.
- Use a different icon library mixed with Lucide.
- Stack absolute-positioned cards inside a fixed-height container — they create
  dead space and z-stack confusion with the next section. Use a grid instead.
- Use `variant="ghost"` for a secondary CTA next to a solid one — pairs badly.
- Let TextLoop animate inside an inline flow — it will shift adjacent text.

---

## Routes & section anchors

| Route | Sections |
| --- | --- |
| `/` | `#modern`, `#conversion`, `#editor`, `#format`, `#intelligent`, `#cta` |
| `/features` | pillars, supports, `#cta` |
| `/download` | hero, `#all-platforms` |
| `/changelog` | hero, release timeline |

Keep navbar/footer links in sync with these anchors. Stale anchors are silent UX bugs.
