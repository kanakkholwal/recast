<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import { Button } from "@recast/ui/button";
	import { AlertTriangle, RotateCcw } from "@lucide/svelte";

	// Inline fallback for a single streamed `{#await}` section that rejected.
	// Keeps the rest of the page usable (filters, forms, sibling sections) while
	// this one degrades in place. "Try again" re-runs the page's load functions
	// via `invalidateAll()`, which recreates the streamed promise — the
	// surrounding `{#await}` then flips back to its pending skeleton and resolves.
	let { message = "Couldn't load this section." }: { message?: string } = $props();

	let retrying = $state(false);
	async function retry() {
		retrying = true;
		try {
			await invalidateAll();
		} finally {
			retrying = false;
		}
	}
</script>

<div class="flex flex-col items-center gap-3 px-4 py-8 text-center">
	<span class="glass-chip grid size-9 place-items-center rounded-lg text-destructive">
		<AlertTriangle class="size-4" />
	</span>
	<p class="text-sm text-muted-foreground">{message}</p>
	<Button size="sm" variant="outline" onclick={retry} disabled={retrying} class="gap-1.5">
		<RotateCcw class={retrying ? "size-3.5 animate-spin" : "size-3.5"} />
		{retrying ? "Retrying…" : "Try again"}
	</Button>
</div>
