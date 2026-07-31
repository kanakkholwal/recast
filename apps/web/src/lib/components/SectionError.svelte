<script lang="ts">
import { Button } from "@recast/ui/button";
import { ArrowLeft, RotateCcw } from "@recast/icons";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { ACCENT_BACKDROP, ACCENT_RING, errorCopy, pickStatusIcon } from "$lib/error/error-copy";

// Compact, shell-preserving error card for section-scoped `+error.svelte`
// boundaries (dashboard, admin). Unlike the full-page global error, this sits
// inside the section's layout so the sidebar + header stay put — a page load
// or streamed-promise rejection degrades in place instead of blowing away the
// whole shell.
let {
	status,
	message = "",
	homeHref,
	homeLabel,
}: {
	status: number;
	message?: string;
	homeHref: string;
	homeLabel: string;
} = $props();

const isServerError = $derived(status >= 500);
const copy = $derived(errorCopy(status, message, isServerError));
const accentRing = $derived(ACCENT_RING[copy.accent]);
const accentBackdrop = $derived(ACCENT_BACKDROP[copy.accent]);
const StatusIcon = $derived(pickStatusIcon(status, isServerError));

// A hard reload is the reliable recovery from a route error boundary (it
// re-runs every load from scratch) and matches the global error page's
// "Try again". `invalidateAll()` doesn't dependably exit a boundary's
// failed state, so we don't use it here.
</script>

<div class="grid min-h-[60vh] place-items-center px-4 py-10">
	<div
		class="glass-card relative w-full max-w-md overflow-hidden rounded-2xl p-7 text-center"
		in:fly={{ y: 16, duration: 420, easing: cubicOut }}
	>
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-x-0 top-0 -z-10 h-32"
			style="background: radial-gradient(ellipse 70% 100% at 50% 0%, {accentBackdrop}, transparent 72%);"
		></div>

		<span class="mx-auto grid size-12 place-items-center rounded-xl ring-1 {accentRing}">
			<StatusIcon class="size-5" />
		</span>

		<p class="mt-5 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
			{copy.eyebrow}
		</p>
		<h1 class="text-balance mt-2 text-xl font-semibold tracking-tight text-foreground">
			{copy.title}
		</h1>
		<p class="text-pretty mx-auto mt-2 max-w-sm text-sm leading-relaxed text-muted-foreground">
			{copy.body}
		</p>

		<div class="mt-6 flex flex-wrap items-center justify-center gap-2.5">
			<Button onclick={() => location.reload()} class="gap-2">
				<RotateCcw class="size-4" />
				Try again
			</Button>
			<Button href={homeHref} variant="outline" class="gap-2">
				<ArrowLeft class="size-4" />
				{homeLabel}
			</Button>
		</div>
	</div>
</div>
