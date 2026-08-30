<script lang="ts">
import { ArrowLeft, RotateCcw } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { ACCENT_BACKDROP, ACCENT_RING, errorCopy, pickStatusIcon } from "$lib/error/error-copy";

// A section-scoped error card that sits inside the layout, so a failed load degrades in place instead of blowing away the shell.
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

// A hard reload is the reliable exit from a route error boundary; `invalidateAll()` doesn't dependably clear one.
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

		<p class="mt-5 text-caption font-medium text-muted-foreground">
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
