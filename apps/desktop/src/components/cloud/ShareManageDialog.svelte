<script lang="ts">
	/**
	 * Manage an existing Recast Cloud share from the desktop: copy/open the
	 * link, change who can view, set/remove a password, set/clear expiry, or
	 * delete the cloud copy. Deleting NEVER touches the local export — that
	 * stays the source of truth.
	 *
	 * Current state is primed from `recast_cloud_list_shares` on open; if that
	 * fails we fall back to write-only defaults (public, no password/expiry).
	 */
	import { recastCloudListShares, type CloudUploadRecord } from "$lib/ipc";
	import { cloudShare } from "$lib/stores/cloudShare.svelte";
	import {
		buildShareUpdate,
		toVisibility,
		type Visibility,
	} from "./share-manage-dialog.logic";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { cn } from "@recast/ui/utils";
	import {
		Check,
		Clock,
		ExternalLink,
		Eye,
		Globe,
		Link2,
		Lock,
		Trash2,
		Users,
	} from "@lucide/svelte";

	let {
		open = false,
		record,
		fileName,
		path,
		onOpenChange,
	}: {
		open?: boolean;
		record: CloudUploadRecord;
		fileName: string;
		path: string;
		onOpenChange?: (open: boolean) => void;
	} = $props();

	function close() {
		onOpenChange?.(false);
	}

	let loading = $state(true);
	let saving = $state(false);
	let deleting = $state(false);
	let views = $state(0);

	// Form state. `password`: "" = unchanged unless `removePassword` is set.
	let visibility = $state<Visibility>("public");
	let initialVisibility = $state<Visibility>("public");
	let hadPassword = $state(false);
	let password = $state("");
	let removePassword = $state(false);
	let expiryDate = $state(""); // yyyy-mm-dd
	let initialExpiry = $state("");

	const VIS: { id: Visibility; label: string; icon: typeof Globe }[] = [
		{ id: "public", label: "Anyone with the link", icon: Globe },
		{ id: "workspace", label: "Only my team", icon: Users },
		{ id: "private", label: "Only me", icon: Lock },
	];

	// Prime from the server when the dialog opens.
	$effect(() => {
		if (!open) return;
		loading = true;
		void prime();
	});

	async function prime() {
		try {
			const res = await recastCloudListShares(record.recastId);
			const row =
				res.shares?.find((s) => s.slug === record.slug) ?? res.shares?.[0];
			if (row) {
				const v = toVisibility(row.visibility);
				visibility = v;
				initialVisibility = v;
				hadPassword = row.hasPassword;
				views = row.viewsCount ?? 0;
				const exp = row.expiresAt ? row.expiresAt.slice(0, 10) : "";
				expiryDate = exp;
				initialExpiry = exp;
			}
		} catch (e) {
			console.error("[cloud] prime manage failed", e);
		} finally {
			loading = false;
		}
	}

	async function copyLink() {
		try {
			await navigator.clipboard.writeText(record.shareUrl);
			toast.success("Share link copied.");
		} catch (e) {
			toast.error(`Couldn't copy: ${e}`);
		}
	}

	async function openLink() {
		try {
			const { openUrl } = await import("@tauri-apps/plugin-opener");
			await openUrl(record.shareUrl);
		} catch {
			window.open(record.shareUrl, "_blank", "noopener");
		}
	}

	async function save() {
		saving = true;
		const opts = buildShareUpdate({
			visibility,
			initialVisibility,
			removePassword,
			password,
			expiryDate,
			initialExpiry,
		});
		if (Object.keys(opts).length === 0) {
			saving = false;
			close();
			return;
		}
		try {
			await cloudShare.updateShare(record.slug, opts);
			toast.success("Share updated.");
			close();
		} catch (e) {
			toast.error(`Couldn't update: ${(e as Error)?.message ?? e}`);
		} finally {
			saving = false;
		}
	}

	async function deleteCloudCopy() {
		deleting = true;
		try {
			await cloudShare.deleteCloud(record.recastId, path);
			toast.success("Cloud copy deleted. Your local file is untouched.");
			close();
		} catch (e) {
			toast.error(`Couldn't delete: ${(e as Error)?.message ?? e}`);
		} finally {
			deleting = false;
		}
	}
</script>

<Dialog.Root {open} onOpenChange={(v) => onOpenChange?.(v)}>
	<Dialog.Content class="max-w-md">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<span class="grid size-7 place-items-center rounded-lg bg-primary/10 text-primary">
					<Link2 class="size-3.5" />
				</span>
				Manage share
			</Dialog.Title>
			<Dialog.Description class="truncate">{fileName}</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4">
			<!-- Link -->
			<div class="flex items-center gap-2">
				<Input value={record.shareUrl} readonly class="h-9 font-mono text-xs" />
				<Button variant="outline" size="sm" class="shrink-0 gap-1.5" onclick={copyLink}>
					<Link2 class="size-3.5" /> Copy
				</Button>
				<Button variant="outline" size="sm" class="shrink-0 gap-1.5" onclick={openLink}>
					<ExternalLink class="size-3.5" /> Open
				</Button>
			</div>

			{#if views > 0}
				<p class="flex items-center gap-1.5 text-xs text-muted-foreground">
					<Eye class="size-3" />
					{views.toLocaleString()} {views === 1 ? "view" : "views"}
				</p>
			{/if}

			<!-- Visibility -->
			<div class="space-y-1.5">
				<Label class="text-xs font-semibold text-foreground/85">Who can view</Label>
				<div class="grid gap-1.5">
					{#each VIS as opt (opt.id)}
						{@const active = visibility === opt.id}
						<button
							type="button"
							onclick={() => (visibility = opt.id)}
							class={cn(
								"flex items-center gap-2.5 rounded-lg border px-3 py-2 text-left text-xs transition-colors",
								active
									? "border-primary/50 bg-primary/8 text-foreground"
									: "border-border-low/60 text-muted-foreground hover:bg-foreground/4",
							)}
						>
							<opt.icon class={cn("size-3.5", active ? "text-primary" : "text-muted-foreground")} />
							<span class="flex-1">{opt.label}</span>
							{#if active}<Check class="size-3.5 text-primary" />{/if}
						</button>
					{/each}
				</div>
			</div>

			<!-- Password -->
			<div class="space-y-1.5">
				<Label class="flex items-center gap-1.5 text-xs font-semibold text-foreground/85">
					<Lock class="size-3" /> Password
				</Label>
				{#if hadPassword && !removePassword}
					<div class="flex items-center justify-between rounded-lg border border-border-low/60 px-3 py-2 text-xs">
						<span class="text-muted-foreground">Password protected</span>
						<button
							type="button"
							class="font-medium text-destructive hover:underline"
							onclick={() => (removePassword = true)}
						>
							Remove
						</button>
					</div>
				{:else}
					<Input
						bind:value={password}
						type="password"
						placeholder={removePassword ? "Password will be removed" : "Set a password (optional)"}
						disabled={removePassword}
						class="h-9"
					/>
					{#if removePassword}
						<button
							type="button"
							class="text-[11px] font-medium text-muted-foreground hover:underline"
							onclick={() => (removePassword = false)}
						>
							Keep existing password
						</button>
					{/if}
				{/if}
			</div>

			<!-- Expiry -->
			<div class="space-y-1.5">
				<Label class="flex items-center gap-1.5 text-xs font-semibold text-foreground/85">
					<Clock class="size-3" /> Link expiry
				</Label>
				<div class="flex items-center gap-2">
					<Input bind:value={expiryDate} type="date" class="h-9" />
					{#if expiryDate}
						<Button variant="ghost" size="sm" onclick={() => (expiryDate = "")}>Clear</Button>
					{/if}
				</div>
			</div>
		</div>

		<Dialog.Footer class="gap-2">
			<Button
				type="button"
				variant="ghost"
				class="mr-auto gap-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive"
				disabled={deleting || saving}
				onclick={deleteCloudCopy}
			>
				<Trash2 class="size-3.5" />
				{deleting ? "Deleting…" : "Delete cloud copy"}
			</Button>
			<Button type="button" variant="ghost" onclick={close}>Cancel</Button>
			<Button type="button" disabled={saving || loading} class="gap-2" onclick={save}>
				{saving ? "Saving…" : "Save"}
				{#if !saving}<Check class="size-4" />{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
