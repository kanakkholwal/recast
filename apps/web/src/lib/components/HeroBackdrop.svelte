<script lang="ts">
  import { scrollRecede } from "$lib/motion-core";

  // The editorial backdrop pattern: full-bleed photo behind a hero, faded at
  // top and bottom so the photo reads edge-to-edge while the page chrome
  // (navbar / footer) stays on clean ground. Extracted from Hero.svelte so
  // every public route can own a different photo with one prop.
  //
  // `tone` covers the three opacities the existing surfaces need:
  //   - default (hero):   opacity-60 dark:opacity-40 — clear photo, room for content
  //   - strong (footer):  opacity-90 dark:opacity-60 — full bleed, photo-first
  //   - subtle (mid):     opacity-50 dark:opacity-30 — atmospheric, not the focus
  //
  // `parallax` opts into scrollRecede on the image div — landing-only polish
  // (a 6% zoom-out as the hero scrolls away) that doesn't earn its cost on the
  // shorter public-route heroes. Default off.

  let {
    src,
    tone = "default",
    parallax = false,
    class: className = "",
  }: {
    src: string;
    tone?: "default" | "strong" | "subtle";
    parallax?: boolean;
    class?: string;
  } = $props();

  const toneClass = {
    default: "opacity-60 dark:opacity-40",
    strong: "opacity-90 dark:opacity-60",
    subtle: "opacity-50 dark:opacity-30",
  } as const;

  let imageEl: HTMLDivElement | undefined;

  // $effect runs after mount in the browser only, so parallax (a scroll-driven
  // effect with no SSR meaning) is the natural place to attach it. Returning
  // the action's destroy handles both effect-re-run and component-destroy.
  $effect(() => {
    if (!parallax || !imageEl) return;
    return scrollRecede(imageEl).destroy;
  });
</script>

<!--
  Three-stop wash that gives the headline a clean reading surface without
  dimming the photo at the top/bottom edges:
    0–18%   — page color, hides the photo under the navbar
    18–34%  — soft fade, photo bleeds in at the top
    34–66%  — 88% page color behind the headline, so dark text reads
               cleanly even on the brightest photo (cloud + green hills)
    66–82%  — soft fade, photo bleeds back at the bottom
    82–100% — page color, hides the photo where the next section starts
  The page is near-black in dark mode, so 88% page color in the middle
  band gives light text a clearly readable surface there too.
-->
<div aria-hidden="true" class={`pointer-events-none absolute inset-0 ${className}`}>
  <div
    bind:this={imageEl}
    class={`absolute inset-0 bg-cover bg-center ${toneClass[tone]}`}
    style={`background-image: url('${src}');`}
  ></div>
  <div
    class="absolute inset-0"
    style="background: linear-gradient(to bottom,
      var(--color-background) 0%,
      transparent 18%,
      color-mix(in srgb, var(--color-background) 88%, transparent) 34%,
      color-mix(in srgb, var(--color-background) 88%, transparent) 66%,
      transparent 82%,
      var(--color-background) 100%);"
  ></div>
</div>