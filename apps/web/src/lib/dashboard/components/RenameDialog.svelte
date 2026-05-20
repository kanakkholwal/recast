<script lang="ts">
	import type { Recast } from "$lib/dashboard/store.svelte";
	import { Button } from "@recast/ui/button";
	import { Check } from "@lucide/svelte";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fade, scale } from "svelte/transition";

	let {
		recast,
		onclose,
		onsave,
	}: {
		recast: Recast;
		onclose: () => void;
		onsave: (title: string) => void;
	} = $props();

	// Seed once — the dialog is freshly mounted per rename, so no need to react.
	let value = $state(untrack(() => recast.title));

	function submit(e: SubmitEvent) {
		e.preventDefault();
		const title = value.trim();
		if (title) onsave(title);
		else onclose();
	}
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="fixed inset-0 z-100 grid place-items-center p-4">
	<button
		type="button"
		aria-label="Cancel"
		onclick={onclose}
		class="absolute inset-0 cursor-default bg-background/80 backdrop-blur-sm"
		transition:fade={{ duration: 150 }}
	></button>

	<form
		onsubmit={submit}
		class="glass-card relative z-10 w-full max-w-sm rounded-2xl p-6 shadow-craft-xl"
		transition:scale={{ start: 0.96, duration: 240, easing: cubicOut }}
	>
		<h2 class="text-sm font-semibold text-foreground">Rename recast</h2>
		<!-- svelte-ignore a11y_autofocus -->
		<input
			type="text"
			bind:value
			autofocus
			class="mt-4 w-full rounded-lg border border-border-low/70 bg-background/80 px-3 py-2.5 text-sm text-foreground outline-none transition-colors focus:border-primary/60"
		/>
		<div class="mt-5 flex justify-end gap-2">
			<Button type="button" variant="ghost" size="sm" onclick={onclose}>
				Cancel
			</Button>
			<Button type="submit" size="sm" class="gap-1.5">
				<Check class="size-3.5" />
				Save
			</Button>
		</div>
	</form>
</div>
