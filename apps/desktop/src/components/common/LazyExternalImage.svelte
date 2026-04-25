<script lang="ts">
  import { resolveAsset } from "$lib/assets";
  import { assetsStore } from "$lib/stores/assets-store.svelte";
  import { Skeleton } from "@recast/ui/skeleton";
  import { onMount, tick } from "svelte";

  interface Props {
    /** Asset id from the manifest. */
    assetId: string;
    alt?: string;
    /** Classes applied to the `<img>` (e.g. `object-cover`). */
    class?: string;
    /**
     * Strict box sizing — the component reserves this space on first paint,
     * before any network I/O, so the skeleton → image transition never shifts
     * surrounding layout. Any CSS `aspect-ratio` value.
     */
    aspectRatio?: string;
    /** Any CSS length; defaults to `100%` so the component fills its parent. */
    width?: string;
    /** Any CSS length; defaults to undefined (derived from `aspectRatio`). */
    height?: string;
    /**
     * Which cached tier to render.
     *  - `"thumb"`: use the WebP thumbnail only — right for grids and pickers.
     *  - `"full"`: use the full-resolution file only — right for hero/preview.
     *  - `"auto"` (default): full-res preferred, thumb as fallback.
     *
     * Grids should pass `"thumb"`. Decoding 23×4K PNGs in a thumbnail picker
     * is what created the original tab-switch jank.
     */
    tier?: "thumb" | "full" | "auto";
  }

  let {
    assetId,
    alt = "",
    class: className = "",
    aspectRatio = "16/9",
    width = "100%",
    height,
    tier = "auto",
  }: Props = $props();

  let online = $state(
    typeof navigator !== "undefined" ? navigator.onLine : true,
  );
  let loaded = $state(false);
  let imgEl: HTMLImageElement | undefined = $state();
  let lastSrc: string | null = null;

  onMount(() => {
    void resolveAsset(assetId);
    const handleOnline = () => (online = true);
    const handleOffline = () => (online = false);
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);
    return () => {
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  });

  // Read the pre-converted URL straight from the store so we never call
  // `convertFileSrc` here — that conversion happens once at `setPath` time.
  // URL identity is stable across re-renders, which is what stops `<img>`
  // elements from re-decoding when surrounding state churns.
  const fullUrl = $derived(assetsStore.urls[assetId]);
  const thumbUrl = $derived(assetsStore.thumbUrls[assetId]);
  const src = $derived.by(() => {
    if (tier === "thumb") return thumbUrl ?? null;
    if (tier === "full") return fullUrl ?? null;
    return fullUrl ?? thumbUrl ?? null;
  });
  const showOfflineBadge = $derived(!src && !online);

  // When `src` actually changes (initial mount, or thumb→full upgrade), check
  // whether the rendered <img> is already complete from the WebView image
  // cache. If yes, promote `loaded` synchronously — `onload` does not refire
  // for an image that arrived already-decoded, which is the failure mode that
  // produced the persistent skeleton flicker on tab return.
  $effect(() => {
    if (src === lastSrc) return;
    lastSrc = src;
    loaded = false;
    if (!src) return;
    void tick().then(() => {
      if (imgEl && imgEl.complete && imgEl.naturalWidth > 0) loaded = true;
    });
  });

  const boxStyle = $derived(
    [
      `width: ${width};`,
      height ? `height: ${height};` : `aspect-ratio: ${aspectRatio};`,
    ].join(" "),
  );
</script>

<span class="relative block overflow-hidden" style={boxStyle}>
  <Skeleton
    class="absolute inset-0 rounded-none transition-opacity duration-200 {loaded
      ? 'opacity-0 pointer-events-none'
      : 'opacity-100'}"
  />

  {#if src}
    <img
      bind:this={imgEl}
      {src}
      {alt}
      class="absolute inset-0 size-full transition-opacity duration-200 {className}"
      style="opacity: {loaded ? 1 : 0};"
      loading="lazy"
      decoding="async"
      draggable="false"
      onload={() => (loaded = true)}
      onerror={() => (loaded = false)}
    />
  {/if}

  {#if showOfflineBadge}
    <span
      class="pointer-events-none absolute right-1 top-1 rounded bg-black/70 px-1 py-0.5 text-[9px] font-medium uppercase tracking-wide text-white"
      aria-label="Offline"
    >
      Offline
    </span>
  {/if}
</span>
