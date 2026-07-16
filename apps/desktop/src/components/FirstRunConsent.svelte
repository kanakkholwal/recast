<script lang="ts">
	import { syncConsent } from "$lib/analytics/client";
	import { desktopConsent } from "$lib/stores/consent.svelte";
	import { Button } from "@recast/ui/button";
	import { cn } from "@recast/ui/utils";
	import { ShieldCheck } from "@recast/icons";

	let { onclose }: { onclose: () => void } = $props();

	// Mirror the store into local reactive state so the switches feel instant;
	// commit on "Continue".
	let product = $state(desktopConsent.product);
	let errors = $state(desktopConsent.errors);

	function done() {
		desktopConsent.setProduct(product);
		desktopConsent.setErrors(errors);
		desktopConsent.markFirstRunSeen();
		// Inits / opts-in / opts-out the live analytics client as needed.
		syncConsent();
		onclose();
	}
</script>

<div
	class="fixed inset-0 z-[100] flex items-center justify-center bg-background/70 backdrop-blur-sm"
	role="dialog"
	aria-modal="true"
	aria-label="Privacy preferences"
>
	<div
		class="mx-4 w-full max-w-md rounded-2xl border border-border/60 bg-card/95 p-6 shadow-xl backdrop-blur"
	>
		<div
			class="flex size-10 items-center justify-center rounded-xl bg-primary/10 text-primary"
		>
			<ShieldCheck class="size-5" />
		</div>
		<h2 class="mt-4 text-[17px] font-semibold tracking-tight text-foreground">
			Help improve Recast
		</h2>
		<p class="mt-1.5 text-[12.5px] leading-relaxed text-muted-foreground">
			Recast is offline-first. Nothing about your recordings ever leaves this
			machine. You choose what diagnostics we may collect, and can change this
			any time in Settings → General.
		</p>

		<div class="mt-5 flex flex-col gap-2">
			<!-- Crash reports: default ON -->
			<button
				type="button"
				role="switch"
				aria-checked={errors}
				aria-label="Anonymous crash reports"
				class="flex items-start justify-between gap-3 rounded-xl border border-border/50 bg-background/40 px-4 py-3 text-left"
				onclick={() => (errors = !errors)}
			>
				<div class="min-w-0">
					<div class="text-[12.5px] font-semibold text-foreground">
						Anonymous crash reports
					</div>
					<div class="text-[11px] text-muted-foreground">
						Send scrubbed error details when something breaks. No file names or
						paths. On by default.
					</div>
				</div>
				<span
					aria-hidden="true"
					class={cn(
						"mt-0.5 flex h-5 w-9 shrink-0 items-center rounded-full transition-colors",
						errors ? "bg-primary" : "bg-input ring-1 ring-inset ring-border/50",
					)}
				>
					<span
						class={cn(
							"size-4 rounded-full bg-card shadow-sm transition-transform",
							errors ? "translate-x-4.5" : "translate-x-0.5",
						)}
					></span>
				</span>
			</button>

			<!-- Product analytics: default OFF -->
			<button
				type="button"
				role="switch"
				aria-checked={product}
				aria-label="Anonymous usage analytics"
				class="flex items-start justify-between gap-3 rounded-xl border border-border/50 bg-background/40 px-4 py-3 text-left"
				onclick={() => (product = !product)}
			>
				<div class="min-w-0">
					<div class="text-[12.5px] font-semibold text-foreground">
						Anonymous usage analytics
					</div>
					<div class="text-[11px] text-muted-foreground">
						Share which features you use so we know what to improve. Off by
						default.
					</div>
				</div>
				<span
					aria-hidden="true"
					class={cn(
						"mt-0.5 flex h-5 w-9 shrink-0 items-center rounded-full transition-colors",
						product ? "bg-primary" : "bg-input ring-1 ring-inset ring-border/50",
					)}
				>
					<span
						class={cn(
							"size-4 rounded-full bg-card shadow-sm transition-transform",
							product ? "translate-x-4.5" : "translate-x-0.5",
						)}
					></span>
				</span>
			</button>
		</div>

		<div class="mt-5 flex justify-end">
			<Button size="sm" onclick={done}>Continue</Button>
		</div>
	</div>
</div>
