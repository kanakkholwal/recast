<script lang="ts">
	import { HeroBackdrop } from "$lib/components";
	import { ArrowLeft } from "@lucide/svelte";

	let { children } = $props();
</script>

<!--
  Shared (auth) layout. Every auth route renders on a single editorial
  backdrop (background-footer.webp) faded so the form is always readable.
  The radial wash behind the form card gives the page a sense of focus
  without going back to the saturated pastels on the landing page.
-->
<div class="relative min-h-screen overflow-hidden text-foreground">
	<HeroBackdrop src="/background-auth.webp" tone="subtle" />
	<!--
	  Soft primary glow under the auth card — anchors the form on the page
	  without competing with the photo.
	-->
	<div
		aria-hidden="true"
		class="pointer-events-none absolute inset-0"
		style="background: radial-gradient(ellipse 60% 50% at 50% 50%, color-mix(in srgb, var(--color-primary) 6%, transparent), transparent 70%);"
	></div>

	<!-- Back-to-site link: glass chip so it stays readable on the photo. -->
	<a
		href="/"
		class="glass-chip absolute left-6 top-6 z-20 inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-semibold text-foreground/80 transition-colors hover:text-foreground"
	>
		<ArrowLeft class="size-3.5" />
		Back to site
	</a>

	<!-- Center the auth card. min-h-screen + grid keeps it vertical-centered
	     even when content is short (login) or tall (verify-email). -->
	<div class="relative z-10 grid min-h-screen place-items-center px-6 py-20">
		{@render children()}
	</div>
</div>