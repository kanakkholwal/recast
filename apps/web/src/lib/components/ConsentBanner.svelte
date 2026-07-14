<script lang="ts">
	import { analytics } from "$lib/analytics/client";
	import { webConsent } from "$lib/analytics/consent.svelte";
	import { Button } from "@recast/ui/button";

	// Basic, anonymous, cookieless metrics already run without this banner. The
	// banner only asks to *upgrade*: persistent profile + session replay.
	function accept() {
		webConsent.accept();
		analytics.upgradePersistence();
	}

	function decline() {
		webConsent.decline();
	}
</script>

{#if webConsent.needsBanner}
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
			<Button size="sm" onclick={accept}>Allow</Button>
		</div>
	</div>
{/if}
