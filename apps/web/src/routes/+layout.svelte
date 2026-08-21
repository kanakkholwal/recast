<script lang="ts">
import { dev } from "$app/environment";
import { navigating, page } from "$app/state";
import { analytics } from "$lib/analytics/client";
import { webConsent } from "$lib/analytics/consent.svelte";
import { authClient } from "$lib/auth/client";
import ImpersonationBanner from "$lib/auth/components/ImpersonationBanner.svelte";
import { AppLoading, DevThemeToggle, Navbar, SeoMeta, ThemeShortcut } from "$lib/components";
import ConsentBanner from "$lib/components/ConsentBanner.svelte";
import { NavProgress } from "@recast/ui/nav-progress";
import { onMount } from "svelte";

import "@recast/application/styles.css";
import "@recast/player/styles.css";
import { Toaster } from "@recast/ui/sonner";
import { ModeWatcher } from "@recast/ui/theme";
import "../app.css";
import {
	buildSiteJsonLd,
	isChromeless as isChromelessPath,
	isIndexable,
	isMarketing as isMarketingPath,
} from "./layout.logic";

let { children } = $props();

onMount(() => {
	const el = document.getElementById("app-splash");
	if (!el) return;
	document.documentElement.classList.add("splash-hydrated");
	const t = setTimeout(() => {
		el.remove();
		document.documentElement.classList.remove(
			"splash-active",
			"splash-hydrated",
			"splash-dashboard",
			"splash-share",
		);
	}, 360);
	return () => clearTimeout(t);
});

const isChromeless = $derived(isChromelessPath(page.url.pathname));
// Gates the border-first token set in app.css. Product shells opt out.
const marketing = $derived(isMarketingPath(page.url.pathname));
const indexable = $derived(isIndexable(page.url.pathname));
const siteJsonLd = $derived(buildSiteJsonLd(page.url.origin));

$effect(() => {
	if (webConsent.hasAccepted) analytics.upgradePersistence();
});

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

<NavProgress active={navigating.to !== null} />
<AppLoading />
<ModeWatcher />
<ThemeShortcut />

<ImpersonationBanner />

{#if !isChromeless}
  <div
    aria-hidden="true"
    class="bg-grid bg-grid-fade pointer-events-none fixed inset-0 -z-10 opacity-70"
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
