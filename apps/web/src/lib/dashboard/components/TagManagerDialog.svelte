<script lang="ts">
	import * as api from "$lib/dashboard/api";
	import { tagsStore, type Tag } from "$lib/dashboard/library.svelte";
	import { recastsStore } from "$lib/dashboard/store.svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import { Check, Tag as TagIcon, Trash2, X } from "@recast/icons";
	import { cubicOut } from "svelte/easing";
	import { fade, scale, slide } from "svelte/transition";

	let { onclose }: { onclose: () => void } = $props();

	// Preset swatches — a clicked dot opens this strip inline. `null` clears.
	const PALETTE = [
		"#ef4444", "#f97316", "#eab308", "#22c55e",
		"#14b8a6", "#3b82f6", "#8b5cf6", "#ec4899",
	];

	// Which row currently has its palette strip open, and which is pending delete.
	let editingColorId = $state<string | null>(null);
	let confirmingId = $state<string | null>(null);

	async function rename(t: Tag, raw: string) {
		const name = raw.trim();
		if (!name || name === t.name) return;
		const prev = t.name;
		tagsStore.update(t.id, { name });
		try {
			await api.updateTag(t.id, { name });
		} catch (e) {
			tagsStore.update(t.id, { name: prev });
			toast.error((e as Error)?.message ?? "Couldn't rename tag.");
		}
	}

	async function recolor(t: Tag, color: string | null) {
		editingColorId = null;
		if (t.color === color) return;
		const prev = t.color;
		tagsStore.update(t.id, { color });
		try {
			await api.updateTag(t.id, { color });
		} catch (e) {
			tagsStore.update(t.id, { color: prev });
			toast.error((e as Error)?.message ?? "Couldn't recolor tag.");
		}
	}

	async function remove(t: Tag) {
		confirmingId = null;
		// Optimistic: drop the chip everywhere and from the tag list.
		const snapshot = { tag: t, taggedRecastIds: recastsStore.items.filter((r) => r.tags.includes(t.id)).map((r) => r.id) };
		tagsStore.remove(t.id);
		recastsStore.removeTagEverywhere(t.id);
		try {
			await api.deleteTag(t.id);
			toast.success(`Tag “${t.name}” deleted.`);
		} catch (e) {
			// Restore the tag and its assignments.
			tagsStore.add(snapshot.tag);
			for (const id of snapshot.taggedRecastIds) {
				const rec = recastsStore.items.find((r) => r.id === id);
				if (rec && !rec.tags.includes(t.id)) recastsStore.setTags(id, [...rec.tags, t.id]);
			}
			toast.error((e as Error)?.message ?? "Couldn't delete tag.");
		}
	}

	function onNameKey(e: KeyboardEvent) {
		if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
		if (e.key === "Escape") onclose();
	}
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="fixed inset-0 z-100 grid place-items-center p-4">
	<button
		type="button"
		aria-label="Close"
		onclick={onclose}
		class="absolute inset-0 cursor-default bg-background/80 backdrop-blur-sm"
		transition:fade={{ duration: 150 }}
	></button>

	<div
		class="glass-card relative z-10 flex max-h-[80vh] w-full max-w-md flex-col rounded-2xl p-6 shadow-craft-xl"
		transition:scale={{ start: 0.96, duration: 240, easing: cubicOut }}
	>
		<div class="flex items-center justify-between">
			<h2 class="flex items-center gap-2 text-sm font-semibold text-foreground">
				<TagIcon class="size-4 text-muted-foreground" />
				Manage tags
			</h2>
			<button
				type="button"
				onclick={onclose}
				aria-label="Close"
				class="grid size-7 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
			>
				<X class="size-4" />
			</button>
		</div>

		{#if tagsStore.sorted.length === 0}
			<p class="mt-6 rounded-lg border border-dashed border-border-low/70 py-10 text-center text-xs text-muted-foreground">
				No tags yet. Create one from the “New tag” button in the filter bar.
			</p>
		{:else}
			<p class="mt-1 text-xs text-muted-foreground">
				Rename inline, recolor with the dot, or delete. Deleting a tag removes it from every recast.
			</p>
			<div class="mt-4 -mr-2 flex flex-col gap-1 overflow-y-auto pr-2">
				{#each tagsStore.sorted as t (t.id)}
					<div class="rounded-lg border border-border-low/50 bg-card/40 px-2 py-1.5">
						<div class="flex items-center gap-2">
							<button
								type="button"
								onclick={() => (editingColorId = editingColorId === t.id ? null : t.id)}
								aria-label="Change color"
								class="grid size-6 shrink-0 place-items-center rounded-md transition-colors hover:bg-foreground/8"
							>
								<span
									class="size-3 rounded-full ring-1 ring-inset ring-border-low/60"
									style={t.color ? `background:${t.color}` : "background:var(--color-muted-foreground); opacity:0.4"}
								></span>
							</button>

							<input
								value={t.name}
								onblur={(e) => rename(t, e.currentTarget.value)}
								onkeydown={onNameKey}
								class="min-w-0 flex-1 rounded-md bg-transparent px-1.5 py-1 text-sm text-foreground outline-none transition-colors focus:bg-background focus:ring-1 focus:ring-primary/40"
							/>

							{#if confirmingId === t.id}
								<div class="flex shrink-0 items-center gap-1" in:slide={{ axis: "x", duration: 160 }}>
									<button
										type="button"
										onclick={() => remove(t)}
										class="rounded-md bg-destructive/15 px-2 py-1 text-[11px] font-semibold text-destructive transition-colors hover:bg-destructive/25"
									>
										Delete
									</button>
									<button
										type="button"
										onclick={() => (confirmingId = null)}
										class="rounded-md px-1.5 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground"
									>
										Cancel
									</button>
								</div>
							{:else}
								<button
									type="button"
									onclick={() => (confirmingId = t.id)}
									aria-label={`Delete ${t.name}`}
									class="grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground/70 transition-colors hover:bg-destructive/10 hover:text-destructive"
								>
									<Trash2 class="size-3.5" />
								</button>
							{/if}
						</div>

						{#if editingColorId === t.id}
							<div class="mt-1.5 flex flex-wrap items-center gap-1.5 pl-8" in:slide={{ duration: 160 }}>
								{#each PALETTE as c (c)}
									<button
										type="button"
										onclick={() => recolor(t, c)}
										aria-label={`Set color ${c}`}
										class="grid size-5 place-items-center rounded-full ring-1 ring-inset ring-border-low/40 transition-transform hover:scale-110"
										style="background:{c}"
									>
										{#if t.color === c}<Check class="size-3 text-white drop-shadow" />{/if}
									</button>
								{/each}
								<button
									type="button"
									onclick={() => recolor(t, null)}
									aria-label="No color"
									class="grid size-5 place-items-center rounded-full bg-foreground/5 text-muted-foreground ring-1 ring-inset ring-border-low/60 transition-colors hover:text-foreground"
								>
									<X class="size-3" />
								</button>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}

		<div class="mt-5 flex justify-end">
			<Button type="button" size="sm" onclick={onclose}>Done</Button>
		</div>
	</div>
</div>
