<script lang="ts">
  import { resolveAsset } from "$lib/assets";
  import { assetsStore } from "$lib/stores/assets-store.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { Image } from "@unpic/svelte";
  import { onMount } from "svelte";

  interface Props {
    /** Asset id from the manifest. */
    assetId: string;
    /** Bundled fallback shown until the cached full-res path resolves. */
    placeholderSrc: string;
    alt?: string;
    class?: string;
  }

  let {
    assetId,
    placeholderSrc,
    alt = "",
    class: className = "",
  }: Props = $props();

  let online = $state(
    typeof navigator !== "undefined" ? navigator.onLine : true,
  );

  // Resolve once on mount in case the store already has it from a prior install
  // that completed before this component subscribed.
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

  const cachedPath = $derived(assetsStore.paths[assetId]);
  const src = $derived(cachedPath ? convertFileSrc(cachedPath) : placeholderSrc);
  const showOfflineBadge = $derived(!cachedPath && !online);
</script>

<span class="relative inline-block size-full">
  <Image
    {src}
    {alt}
    class={className}
    loading="lazy"
    decoding="async"
  />
  {#if showOfflineBadge}
    <span
      class="pointer-events-none absolute right-1 top-1 rounded bg-black/70 px-1 py-0.5 text-[9px] font-medium uppercase tracking-wide text-white"
      aria-label="Offline"
    >
      Offline
    </span>
  {/if}
</span>
