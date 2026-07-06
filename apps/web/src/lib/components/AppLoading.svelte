<script lang="ts">
	import { navigating } from "$app/state";
	import Logo from "$lib/logo.svelte";
	import { isAppArea } from "../../routes/layout.logic";
	import { fade } from "svelte/transition";

	// Branded, full-surface loading screen for client-side navigations *into* a
	// product area (SvelteKit's answer to Next's loading.tsx). The thin
	// <NavProgress /> bar still runs everywhere; this fuller loader only appears
	// for /dashboard and /share/* targets. A short delay keeps instant
	// navigations from flashing it.
	const SHOW_DELAY_MS = 160;

	let show = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => {
		const target = navigating.to?.url.pathname;
		const active = Boolean(target && isAppArea(target));
		clearTimeout(timer);
		if (active) {
			timer = setTimeout(() => (show = true), SHOW_DELAY_MS);
		} else {
			show = false;
		}
		return () => clearTimeout(timer);
	});
</script>

{#if show}
	<div
		class="fixed inset-0 z-[70] grid place-items-center bg-background/80 backdrop-blur-md"
		role="status"
		aria-live="polite"
		aria-label="Loading"
		transition:fade={{ duration: 150 }}
	>
		<div class="flex flex-col items-center gap-4">
			<span class="app-loading-mark relative grid size-14 place-items-center rounded-2xl bg-foreground text-background shadow-craft-lg">
				<span class="app-loading-ring pointer-events-none absolute -inset-2 rounded-full" aria-hidden="true"></span>
				<Logo size="30" color="transparent" fill="currentColor" />
			</span>
			<span class="text-xs font-medium text-muted-foreground">Loading…</span>
		</div>
	</div>
{/if}

<style>
	.app-loading-mark {
		animation: app-loading-pulse 1.4s ease-in-out infinite;
	}
	.app-loading-ring {
		border: 2px solid transparent;
		border-top-color: var(--color-primary, #cdec3a);
		animation: app-loading-spin 0.9s linear infinite;
	}
	@keyframes app-loading-pulse {
		0%,
		100% {
			transform: scale(1);
		}
		50% {
			transform: scale(1.06);
		}
	}
	@keyframes app-loading-spin {
		to {
			transform: rotate(360deg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.app-loading-mark,
		.app-loading-ring {
			animation: none;
		}
	}
</style>
