<script lang="ts">
  import { resolveAsset } from "$lib/assets";
  import { assetsStore } from "$lib/stores/assets-store.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { Skeleton } from "@recast/ui/skeleton";
  import { onMount } from "svelte";

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
  }

  let {
    assetId,
    alt = "",
    class: className = "",
    aspectRatio = "16/9",
    width = "100%",
    height,
  }: Props = $props();

  let online = $state(
    typeof navigator !== "undefined" ? navigator.onLine : true,
  );
  let loaded = $state(false);

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

  const fullPath = $derived(assetsStore.paths[assetId]);
  const thumbPath = $derived(assetsStore.thumbPaths[assetId]);
  const resolvedPath = $derived(fullPath ?? thumbPath);
  const src = $derived(resolvedPath ? convertFileSrc(resolvedPath) : null);
  const showOfflineBadge = $derived(!resolvedPath && !online);

  // Reset the decoded flag when the src swaps (thumb → full-res upgrade) so
  // the skeleton reappears briefly under the second decode.
  $effect(() => {
    void src;
    loaded = false;
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
      {src}
      {alt}
      class="absolute inset-0 size-full transition-opacity duration-200 {className}"
      style="opacity: {loaded ? 1 : 0};"
      loading="lazy"
      decoding="async"
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
