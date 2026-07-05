<script lang="ts">
  import { dev } from "$app/environment";
  import { page } from "$app/state";
  import { analytics } from "$lib/analytics/client";
  import { webConsent } from "$lib/analytics/consent.svelte";
  import { authClient } from "$lib/auth/client";
  import ImpersonationBanner from "$lib/auth/components/ImpersonationBanner.svelte";
  import {
    DevThemeToggle,
    Navbar,
    SeoMeta,
    ThemeShortcut,
  } from "$lib/components";
  import ConsentBanner from "$lib/components/ConsentBanner.svelte";
  import { NavProgress } from "@recast/ui/nav-progress";

  import { Toaster } from "@recast/ui/sonner";
  import { ModeWatcher } from "@recast/ui/theme";
  import {
    buildSiteJsonLd,
    isChromeless as isChromelessPath,
    isIndexable,
  } from "./layout.logic";
  import "../app.css";
// Player theme — imported once at the root so any /share or dashboard
  // route that mounts <RecastPlayer> picks up the branded CSS variables.
  import "@recast/player/styles.css";

  let { children } = $props();

  const isChromeless = $derived(isChromelessPath(page.url.pathname));
  const indexable = $derived(isIndexable(page.url.pathname));
  const siteJsonLd = $derived(buildSiteJsonLd(page.url.origin));

  // Returning visitor who already accepted → re-enable replay + persistent id
  // before any events fire this session.
  $effect(() => {
    if (webConsent.hasAccepted) analytics.upgradePersistence();
  });

  // Tie events to the signed-in user (aliases the anonymous distinct id) and
  // drop the identity on sign-out. Gated by product consent inside the client.
  const session = authClient.useSession();
  let lastUserId: string | null = null;
  $effect(() => {
    const userId = $session.data?.user?.id ?? null;
    if (userId && userId !== lastUserId) {
      analytics.identify(userId);
      lastUserId = userId;
    } else if (!userId && lastUserId) {
      analytics.reset();
      lastUserId = null;
    }
  });
</script>

<!-- Site-wide default social/SEO tags. Routes that need their own card (e.g.
	 a shared recast) set `customSeo: true` in their load data and render their
	 own <SeoMeta>; suppressing the default here keeps a single, authoritative
	 set of og: tags instead of duplicates (scrapers take the first og:image). -->
{#if !(page.data as { customSeo?: boolean }).customSeo}
  <SeoMeta
    title="Record. Polish. Share."
    description="Recast turns a raw screen capture into a polished, shareable demo. Smart auto-edits and a friendly timeline anyone can drive. macOS, Windows, Linux."
    pageTitle="Recast - Record. Polish. Share."
  />
{/if}
<svelte:head>
  {#if indexable}
    {@html `<script type="application/ld+json">${siteJsonLd}</` + `script>`}
  {:else}
    <meta name="robots" content="noindex, nofollow" />
  {/if}
</svelte:head>
<NavProgress />
<ModeWatcher />
<!-- Cmd/Ctrl+Shift+L from any route toggles light↔dark. Runs in prod
	 (DevThemeToggle's floating chip is still dev-only). -->
<ThemeShortcut />

<!-- Global impersonation indicator. Self-renders only when an admin is
	 acting as another user; invisible otherwise. -->
<ImpersonationBanner />

{#if !isChromeless}
  <div
    aria-hidden="true"
    class="bg-grid bg-grid-fade pointer-events-none fixed inset-0 -z-10 opacity-30"
  ></div>

  <Navbar />
{/if}

<div class="relative isolate flex min-h-screen flex-col overflow-x-hidden">
  {@render children()}
</div>

<Toaster position="bottom-right" duration={5000} />

<ConsentBanner />

{#if dev}
  <DevThemeToggle />
{/if}
