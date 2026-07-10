<script lang="ts">
	/**
	 * Pick which workspace a recast uploads into, shown at share time when the
	 * user belongs to more than one. Mirrors the web's per-upload `workspaceId`
	 * contract. The choice is passed straight to `recast_cloud_upload`, and the
	 * server re-validates membership on `/api/uploads/init`. Optionally remembers
	 * the pick as the desktop's default (a local preference; it never touches the
	 * web session's active org).
	 */
	import type { CloudWorkspace } from "$lib/stores/cloudShare.svelte";
	import { planLabel, roleLabel } from "$components/settings/cloud-signin.logic";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Label } from "@recast/ui/label";
	import { cn } from "@recast/ui/utils";
	import { Check, Crown, Send, Users } from "@lucide/svelte";

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

	// Re-seed the selection each time the dialog opens so a prior cancel doesn't
	// leak into the next share. Defaults to the active workspace, else the first.
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

<Dialog.Root {open} onOpenChange={(v) => onOpenChange?.(v)}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<span class="grid size-7 place-items-center rounded-lg bg-primary/10 text-primary">
					<Users class="size-3.5" />
				</span>
				Share to which workspace?
			</Dialog.Title>
			<Dialog.Description class="truncate">{fileName}</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-3">
			<div class="grid max-h-64 gap-1.5 overflow-y-auto">
				{#each workspaces as ws (ws.id)}
					{@const active = chosen === ws.id}
					{@const isOwner = ws.role === "owner"}
					{@const isPaid = ws.plan !== "free"}
					<button
						type="button"
						onclick={() => (chosen = ws.id)}
						class={cn(
							"flex items-center gap-2.5 rounded-lg border px-3 py-2.5 text-left text-xs transition-colors",
							active
								? "border-primary/50 bg-primary/8 text-foreground"
								: "border-border-low/60 text-muted-foreground hover:bg-foreground/4",
						)}
					>
						<span
							class={cn(
								"grid size-7 shrink-0 place-items-center rounded-md text-[11px] font-semibold",
								active ? "bg-primary/15 text-primary" : "bg-muted text-muted-foreground",
							)}
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
						{#if active}<Check class="size-4 shrink-0 text-primary" />{/if}
					</button>
				{/each}
			</div>

			<button
				type="button"
				onclick={() => (remember = !remember)}
				class="flex w-full items-center gap-2.5 rounded-lg px-1 py-1 text-left"
			>
				<span
					class={cn(
						"grid size-4 shrink-0 place-items-center rounded border transition-colors",
						remember ? "border-primary bg-primary text-primary-foreground" : "border-border",
					)}
				>
					{#if remember}<Check class="size-3" />{/if}
				</span>
				<Label class="cursor-pointer text-xs text-muted-foreground">
					Always upload here (set as default)
				</Label>
			</button>
		</div>

		<Dialog.Footer class="gap-2">
			<Button type="button" variant="ghost" onclick={() => onOpenChange?.(false)}>
				Cancel
			</Button>
			<Button type="button" class="gap-2" disabled={!chosen} onclick={confirm}>
				Share here
				<Send class="size-3.5" />
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
