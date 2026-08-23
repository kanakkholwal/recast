<script lang="ts">
import { Check, Trash2, X } from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as Dialog from "@recast/ui/dialog";
import { toast } from "@recast/ui/sonner";
import { slide } from "svelte/transition";
import * as api from "$lib/dashboard/api";
import { type Tag, tagsStore } from "$lib/dashboard/library.svelte";
import { recastsStore } from "$lib/dashboard/store.svelte";

let { onclose }: { onclose: () => void } = $props();

// Preset swatches — a clicked dot opens this strip inline. `null` clears.
const PALETTE = [
	"#ef4444",
	"#f97316",
	"#eab308",
	"#22c55e",
	"#14b8a6",
	"#3b82f6",
	"#8b5cf6",
	"#ec4899",
];

// Which row currently has its palette strip open, and which is pending delete.
let editingColorId = $state<string | null>(null);
let confirmingId = $state<string | null>(null);

function countFor(tagId: string): number {
	return recastsStore.items.filter((r) => r.tags.includes(tagId)).length;
}

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
	const snapshot = {
		tag: t,
		taggedRecastIds: recastsStore.items.filter((r) => r.tags.includes(t.id)).map((r) => r.id),
	};
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
}
</script>

<Dialog.Root
	open
	onOpenChange={(next) => {
		if (!next) onclose();
	}}
>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Manage tags</Dialog.Title>
			<Dialog.Description>
				Rename inline, recolour with the dot, or delete. Deleting removes the tag from every recast.
			</Dialog.Description>
		</Dialog.Header>

		{#if tagsStore.sorted.length === 0}
			<p
				class="rounded-lg border border-dashed border-border-low py-10 text-center text-body-sm text-muted-foreground"
			>
				No tags yet. Create one from the “New tag” button in the filter bar.
			</p>
		{:else}
			<div class="-mr-2 flex max-h-[50vh] flex-col overflow-y-auto pr-2">
				{#each tagsStore.sorted as t (t.id)}
					{@const used = countFor(t.id)}
					<div class="border-b border-border-low py-1.5 last:border-b-0">
						<div class="flex items-center gap-2">
							<button
								type="button"
								onclick={() => (editingColorId = editingColorId === t.id ? null : t.id)}
								aria-label="Change colour"
								class="grid size-6 shrink-0 place-items-center rounded-md transition-colors hover:bg-paper motion-reduce:transition-none"
							>
								<span
									class="size-3 rounded-full ring-1 ring-inset ring-border-low"
									style={t.color
										? `background:${t.color}`
										: "background:var(--color-border-strong)"}
								></span>
							</button>

							<input
								value={t.name}
								onblur={(e) => rename(t, e.currentTarget.value)}
								onkeydown={onNameKey}
								aria-label="Tag name"
								class="min-w-0 flex-1 rounded-md bg-transparent px-1.5 py-1 text-body-sm text-foreground outline-none transition-colors focus:bg-paper motion-reduce:transition-none"
							/>

							<span class="shrink-0 text-caption tabular-nums text-muted-foreground">
								{used}
							</span>

							{#if confirmingId === t.id}
								<div
									class="flex shrink-0 items-center gap-1"
									in:slide={{ axis: "x", duration: 160 }}
								>
									<Button
										variant="destructive"
										size="xs"
										onclick={() => remove(t)}
										class="h-7 px-2"
									>
										Delete
									</Button>
									<Button
										variant="ghost"
										size="xs"
										onclick={() => (confirmingId = null)}
										class="h-7 px-2"
									>
										Cancel
									</Button>
								</div>
							{:else}
								<button
									type="button"
									onclick={() => (confirmingId = t.id)}
									aria-label={`Delete ${t.name}`}
									class="grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive motion-reduce:transition-none"
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
										aria-label={`Set colour ${c}`}
										class="grid size-5 place-items-center rounded-full ring-1 ring-inset ring-border-low transition-transform hover:scale-110 motion-reduce:transition-none"
										style="background:{c}"
									>
										{#if t.color === c}<Check class="size-3 text-white" />{/if}
									</button>
								{/each}
								<button
									type="button"
									onclick={() => recolor(t, null)}
									aria-label="No colour"
									class="grid size-5 place-items-center rounded-full bg-paper text-muted-foreground ring-1 ring-inset ring-border-low transition-colors hover:text-foreground motion-reduce:transition-none"
								>
									<X class="size-3" />
								</button>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}

		<Dialog.Footer>
			<Button type="button" size="sm" variant="dark" onclick={onclose}>Done</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
