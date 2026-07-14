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

<div aria-hidden="true" class={`pointer-events-none absolute inset-0 ${className}`}>
  <div
    bind:this={imageEl}
    class={`absolute inset-0 bg-cover bg-center ${toneClass[tone]}`}
    style={`background-image: url('${src}');`}
  ></div>
  <div
    class="absolute inset-0"
    style="background: linear-gradient(to bottom, var(--color-background) 0%, transparent 45%, transparent 78%, var(--color-background) 100%);"
  ></div>
</div>