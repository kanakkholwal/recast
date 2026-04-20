<script lang="ts">
  import { resolveAsset } from "$lib/assets";
  import { assetsStore } from "$lib/stores/assets-store.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  interface Props {
    /** Asset id from the manifest. */
    assetId: string;
    alt?: string;
    class?: string;
    /** CSS value for the fallback surface shown before any cache tier resolves. */
    placeholderBackground?: string;
  }

  let {
    assetId,
    alt = "",
    class: className = "",
    placeholderBackground = "linear-gradient(135deg, oklch(0.28 0.03 260) 0%, oklch(0.22 0.04 300) 100%)",
  }: Props = $props();

  let online = $state(
    typeof navigator !== "undefined" ? navigator.onLine : true,
  );

  onMount(() => {
    // Kick in case the store isn't populated yet for this id.
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
</script>

<span class="relative inline-block size-full overflow-hidden">
  {#if src}
    <img
      {src}
      {alt}
      class={className}
      loading="lazy"
      decoding="async"
    />
  {:else}
    <span
      class="flex size-full items-center justify-center {className}"
      style="background: {placeholderBackground};"
      role="img"
      aria-label={alt || "Loading wallpaper"}
    ></span>
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
