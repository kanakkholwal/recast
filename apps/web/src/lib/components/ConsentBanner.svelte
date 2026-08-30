<script lang="ts">
import { Button } from "@recast/ui/button";
import { onMount } from "svelte";
import { analytics } from "$lib/analytics/client";
import { webConsent } from "$lib/analytics/consent.svelte";

let showBanner = $state(false);

// Anonymous cookieless metrics already run; the banner only asks to upgrade to a profile and session replay.
function accept() {
	webConsent.accept();
	analytics.upgradePersistence();
}

function decline() {
	webConsent.decline();
}

onMount(() => {
	const timeout = window.setTimeout(() => {
		showBanner = true;
	}, 5000);

	return () => window.clearTimeout(timeout);
});
</script>

{#if webConsent.needsBanner && showBanner}
  <div
    class="fixed bottom-4 left-4 z-50 max-w-sm rounded-lg border border-border bg-popover p-4 text-popover-foreground shadow-lg fade-in slide-in delay-3000"
    role="dialog"
    aria-label="Privacy preferences"
  >
    <p class="text-sm font-medium">We respect your privacy</p>
    <p class="mt-1 text-sm text-muted-foreground">
      We collect anonymous, cookieless usage metrics to improve Recast. Allow
      cookies to enable session replay and a saved profile, or keep it minimal.
    </p>
    <div class="mt-3 flex justify-end gap-2">
      <Button variant="ghost" size="sm" onclick={decline}>Keep minimal</Button>
      <Button size="sm" variant="dark" onclick={accept}>Allow</Button>
    </div>
  </div>
{/if}
