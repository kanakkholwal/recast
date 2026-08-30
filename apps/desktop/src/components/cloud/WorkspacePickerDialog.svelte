<script lang="ts">
/**
 * Pick which workspace a recast uploads into, shown at share time when the
 * user belongs to more than one. Mirrors the web's per-upload `workspaceId`
 * contract. The choice is passed straight to `recast_cloud_upload`, and the
 * server re-validates membership on `/api/uploads/init`. Optionally remembers
 * the pick as the desktop's default (a local preference; it never touches the
 * web session's active org).
 */

import DialogShell from "@recast/editor/components/dialog/DialogShell.svelte";
import { Check, Crown, Send, Users } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import { planLabel, roleLabel } from "$components/settings/cloud-signin.logic";
import type { CloudWorkspace } from "$lib/stores/cloudShare.svelte";

let {
	open = false,
	workspaces,
	activeId,
	fileName,
	onConfirm,
	onOpenChange,
}: {
	open?: boolean;
	workspaces: CloudWorkspace[];
	/** The currently-resolved default, pre-selected on open. */
	activeId: string | null;
	fileName: string;
	/** Fires with the chosen workspace + whether to persist it as default. */
	onConfirm: (workspaceId: string, remember: boolean) => void;
	onOpenChange?: (open: boolean) => void;
} = $props();

let chosen = $state<string | null>(null);
let remember = $state(false);

// Re-seeded on open so a prior cancel can't leak into the next share; defaults to the active workspace, else the first.
$effect(() => {
	if (open) {
		chosen = activeId ?? workspaces[0]?.id ?? null;
		remember = false;
	}
});

function confirm() {
	if (!chosen) return;
	onConfirm(chosen, remember);
	onOpenChange?.(false);
}
</script>

<DialogShell
	{open}
	title="Share to which workspace?"
	subtitle={fileName}
	icon={Users}
	onOpenChange={(v) => onOpenChange?.(v)}
>
	<div class="space-y-3">
		<!-- A radiogroup, not a row of buttons: exactly one is chosen, and the tick
		     was the only thing saying which — invisible to a screen reader. -->
		<div class="grid max-h-64 gap-1.5 overflow-y-auto" role="radiogroup" aria-label="Workspace">
			{#each workspaces as ws (ws.id)}
				{@const active = chosen === ws.id}
				{@const isOwner = ws.role === "owner"}
				{@const isPaid = ws.plan !== "free"}
				<button
					type="button"
					role="radio"
					aria-checked={active}
					onclick={() => (chosen = ws.id)}
					class={cn(
						"flex items-center gap-2.5 rounded-lg border px-3 py-2.5 text-left text-xs transition-colors",
						active
							? "border-foreground/40 bg-foreground/5 text-foreground"
							: "border-border-low/60 text-muted-foreground hover:bg-foreground/4",
					)}
				>
					<span
						class={cn(
							"grid size-7 shrink-0 place-items-center rounded-md text-[11px] font-semibold",
							active ? "bg-foreground text-background" : "bg-muted text-muted-foreground",
						)}
						aria-hidden="true"
					>
						{(ws.name.trim()[0] ?? "?").toUpperCase()}
					</span>
					<span class="min-w-0 flex-1">
						<span class="flex items-center gap-1.5">
							<span class="truncate font-medium text-foreground">{ws.name}</span>
							<span
								class={cn(
									"inline-flex shrink-0 items-center gap-0.5 rounded-full px-1.5 py-px text-[9px] font-bold uppercase tracking-wide",
									isPaid
										? "bg-primary/10 text-primary ring-1 ring-inset ring-primary/30"
										: "bg-muted text-muted-foreground ring-1 ring-inset ring-border/50",
								)}
							>
								{#if isPaid}<Crown class="size-2" />{/if}
								{planLabel(ws.plan)}
							</span>
						</span>
						<span class="flex items-center gap-1.5 text-[10.5px] text-muted-foreground">
							<span class="flex items-center gap-1">
								{#if isOwner}<Crown class="size-2.5" />{/if}
								{roleLabel(ws.role)}
							</span>
							<span class="text-muted-foreground/40">·</span>
							<span>{ws.recastsCount} {ws.recastsCount === 1 ? "recast" : "recasts"}</span>
						</span>
					</span>
					{#if active}<Check class="size-4 shrink-0 text-foreground" aria-hidden="true" />{/if}
				</button>
			{/each}
		</div>

		<!-- Was a <button> wrapping a <Label>, with its on/off state exposed
		     nowhere. A real checkbox says what it is and what it's set to. -->
		<label class="flex w-full cursor-pointer items-center gap-2.5 px-1 py-1 text-left">
			<input type="checkbox" bind:checked={remember} class="peer sr-only" />
			<span
				class={cn(
					"grid size-4 shrink-0 place-items-center rounded border transition-colors",
					"peer-focus-visible:ring-2 peer-focus-visible:ring-ring/60",
					remember ? "border-foreground bg-foreground text-background" : "border-border",
				)}
				aria-hidden="true"
			>
				{#if remember}<Check class="size-3" />{/if}
			</span>
			<span class="text-xs text-muted-foreground">Always upload here (set as default)</span>
		</label>
	</div>

	{#snippet footer()}
		<Button type="button" variant="ghost" size="xs" onclick={() => onOpenChange?.(false)}>
			Cancel
		</Button>
		<Button type="button" size="xs" class="gap-2" disabled={!chosen} onclick={confirm}>
			Share here
			<Send class="size-3.5" />
		</Button>
	{/snippet}
</DialogShell>
