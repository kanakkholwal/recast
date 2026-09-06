<script lang="ts">
import { AlertTriangle, RotateCcw } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { invalidateAll } from "$app/navigation";

// Keeps the rest of the page usable while one section degrades; Try again re-runs the loads, so the `{#await}` returns to pending.
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
