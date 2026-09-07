<script lang="ts">
import { fade } from "svelte/transition";
import { navigating } from "$app/state";
import Logo from "$lib/logo.svelte";
import { isAppArea } from "../../routes/layout.logic";

// The fuller loader for navigations into a product area, on the same plate as app.html's splash and the desktop boot screen; app.html inlines its copy, so the two move together by hand.
const SHOW_DELAY_MS = 160;

let show = $state(false);
let timer: ReturnType<typeof setTimeout> | undefined;

const target = $derived(navigating.to?.url.pathname ?? "");
const status = $derived(
	target.startsWith("/share/") ? "Preparing the player" : "Opening your workspace",
);

$effect(() => {
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
		class="fixed inset-0 z-[70] grid place-items-center bg-background/85 backdrop-blur-md"
		role="status"
		aria-live="polite"
		transition:fade={{ duration: 150 }}
	>
		<div class="app-loading-plate flex flex-col items-center gap-3.5 p-8">
			<span
				class="app-loading-mark grid size-14 place-items-center rounded-2xl bg-foreground text-background shadow-craft-lg"
			>
				<Logo size="30" color="transparent" fill="currentColor" />
			</span>
			<span class="text-xl font-semibold leading-none tracking-tight text-foreground">
				Recast
			</span>
			<span
				class="font-mono text-[11px] uppercase leading-none tracking-[0.18em] text-muted-foreground"
			>
				Record · Polish · Share
			</span>
			<span class="-mt-0.5 text-[12.5px] leading-none text-muted-foreground">{status}</span>
			<span class="app-loading-bar mt-0.5" aria-hidden="true"></span>
		</div>
	</div>
{/if}

<style>
	.app-loading-plate {
		animation: app-loading-plate-in 380ms cubic-bezier(0.16, 1, 0.3, 1) both;
	}
	.app-loading-mark {
		animation: app-loading-pop 400ms cubic-bezier(0.2, 0.7, 0.3, 1) both;
	}
	.app-loading-bar {
		position: relative;
		width: 132px;
		height: 3px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--color-primary) 16%, transparent);
		overflow: hidden;
	}
	.app-loading-bar::after {
		content: "";
		position: absolute;
		inset-block: 0;
		left: 0;
		width: 40%;
		border-radius: inherit;
		background: linear-gradient(90deg, transparent, var(--color-primary), transparent);
		animation: app-loading-shimmer 1.4s cubic-bezier(0.4, 0, 0.2, 1) infinite;
		will-change: transform;
	}
	@keyframes app-loading-shimmer {
		from { transform: translateX(-120%); }
		to { transform: translateX(350%); }
	}
	@keyframes app-loading-pop {
		from { transform: scale(0.88); opacity: 0; }
		to { transform: scale(1); opacity: 1; }
	}
	@keyframes app-loading-plate-in {
		from { transform: translateY(4px); opacity: 0; }
		to { transform: translateY(0); opacity: 1; }
	}
	@media (prefers-reduced-motion: reduce) {
		.app-loading-plate,
		.app-loading-mark {
			animation: none;
		}
		.app-loading-bar::after {
			width: 100%;
			background: var(--color-primary);
			opacity: 0.85;
			animation: none;
		}
	}
</style>
