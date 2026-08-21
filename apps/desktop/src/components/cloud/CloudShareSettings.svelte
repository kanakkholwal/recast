<script lang="ts">
/**
 * Shared Recast Cloud share controls: the link (copy/open), viewer count,
 * visibility, password and expiry. Laid out to match the web QuickUpload
 * configure step (Select-based inputs, an Options card, roomy spacing). Used
 * both by ShareManageDialog and inline in the upload flow (CloudShareDialog).
 * Primes current state from the server; the parent applies via bound `save`.
 */
import { recastCloudListShares } from "$lib/ipc";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import type { IconComponent } from "@recast/icons";
import {
	CalendarClock,
	ExternalLink,
	Eye,
	Globe,
	KeyRound,
	Link2,
	Lock,
	Users,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Input } from "@recast/ui/input";
import * as Select from "@recast/ui/select";
import { toast } from "@recast/ui/sonner";
import { buildShareUpdate, toVisibility, type Visibility } from "./share-manage-dialog.logic";

let {
	recastId,
	slug,
	shareUrl,
	save = $bindable(async () => true),
	saving = $bindable(false),
	loading = $bindable(true),
}: {
	recastId: string;
	slug: string;
	shareUrl: string;
	/** Bound by the parent; applies only changed fields, returns success. */
	save?: () => Promise<boolean>;
	saving?: boolean;
	loading?: boolean;
} = $props();

type ExpiryChoice = "keep" | "never" | "7d" | "30d";

let views = $state(0);
// Set until the server's current settings are known. Every control below
// defaults to "public / no password / never", so showing the form before the
// prime lands would misreport a private link and make "Remove password"
// unreachable — `buildShareUpdate` would diff against the wrong baseline.
let primeError = $state<string | null>(null);
let visibility = $state<Visibility>("public");
let initialVisibility = $state<Visibility>("public");
let hadPassword = $state(false);
let password = $state("");
let removePassword = $state(false);
let initialExpiry = $state(""); // yyyy-mm-dd, "" = no expiry
let expiryChoice = $state<ExpiryChoice>("never");

const VIS: { id: Visibility; label: string; icon: IconComponent }[] = [
	{ id: "public", label: "Anyone with the link", icon: Globe },
	{ id: "workspace", label: "Only my team", icon: Users },
	{ id: "private", label: "Only me", icon: Lock },
];
const visMeta = $derived(VIS.find((v) => v.id === visibility) ?? VIS[0]);
const hasExistingExpiry = $derived(initialExpiry !== "");

const expiryLabel = $derived(
	expiryChoice === "keep"
		? `Keep until ${initialExpiry}`
		: expiryChoice === "7d"
			? "7 days"
			: expiryChoice === "30d"
				? "30 days"
				: "Never",
);

$effect(() => {
	void slug;
	loading = true;
	void prime();
});

async function prime() {
	primeError = null;
	try {
		const res = await recastCloudListShares(recastId);
		const row = res.shares?.find((s) => s.slug === slug) ?? res.shares?.[0];
		if (!row) {
			primeError = "This share is no longer on the server.";
			return;
		}
		const v = toVisibility(row.visibility);
		visibility = v;
		initialVisibility = v;
		hadPassword = row.hasPassword;
		views = row.viewsCount ?? 0;
		initialExpiry = row.expiresAt ? row.expiresAt.slice(0, 10) : "";
		expiryChoice = initialExpiry ? "keep" : "never";
	} catch (e) {
		console.error("[cloud] prime share settings failed", e);
		primeError = (e as Error)?.message ?? String(e);
	} finally {
		loading = false;
	}
}

function retryPrime() {
	loading = true;
	void prime();
}

function expiryDate(): string {
	if (expiryChoice === "keep") return initialExpiry;
	if (expiryChoice === "never") return "";
	const days = expiryChoice === "7d" ? 7 : 30;
	return new Date(Date.now() + days * 86_400_000).toISOString().slice(0, 10);
}

// Exposed to the parent's primary button. Applies only changed fields.
save = async () => {
	if (primeError) {
		toast.error("Can't save: the current share settings couldn't be loaded.");
		return false;
	}
	saving = true;
	const opts = buildShareUpdate({
		visibility,
		initialVisibility,
		removePassword,
		password,
		expiryDate: expiryDate(),
		initialExpiry,
	});
	if (Object.keys(opts).length === 0) {
		saving = false;
		return true;
	}
	try {
		await cloudShare.updateShare(slug, opts);
		toast.success("Share updated.");
		return true;
	} catch (e) {
		toast.error(`Couldn't update: ${(e as Error)?.message ?? e}`);
		return false;
	} finally {
		saving = false;
	}
};

async function copyLink() {
	try {
		await navigator.clipboard.writeText(shareUrl);
		toast.success("Share link copied.");
	} catch (e) {
		toast.error(`Couldn't copy: ${e}`);
	}
}
async function openLink() {
	try {
		const { openUrl } = await import("@tauri-apps/plugin-opener");
		await openUrl(shareUrl);
	} catch {
		window.open(shareUrl, "_blank", "noopener");
	}
}
</script>

<div class="space-y-5">
	<!-- Link -->
	<div class="flex items-center gap-2">
		<Input value={shareUrl} readonly class="h-9 font-mono text-xs" />
		<Button
			variant="outline"
			size="sm"
			class="h-9 shrink-0 gap-1.5"
			onclick={copyLink}
		>
			<Link2 class="size-3.5" /> Copy
		</Button>
		<Button
			variant="outline"
			size="sm"
			class="h-9 shrink-0 gap-1.5"
			onclick={openLink}
		>
			<ExternalLink class="size-3.5" /> Open
		</Button>
	</div>

	{#if primeError}
		<div
			class="flex flex-col gap-2 rounded-lg border border-destructive/40 bg-destructive/5 px-3 py-3"
			role="alert"
		>
			<p class="text-sm font-medium text-foreground">Couldn't load these share settings</p>
			<p class="text-xs leading-relaxed text-muted-foreground">
				{primeError} The link above still works — nothing has changed.
			</p>
			<Button variant="outline" size="sm" class="self-start" onclick={retryPrime}>
				Try again
			</Button>
		</div>
	{:else}
	{#if views > 0}
		<p class="flex items-center gap-1.5 text-xs text-muted-foreground">
			<Eye class="size-3.5" />
			{views.toLocaleString()}
			{views === 1 ? "view" : "views"}
		</p>
	{/if}

	<!-- Visibility -->
	<section>
		<h3 class="mb-1.5 text-sm font-semibold text-foreground">Who can view</h3>
		<Select.Root type="single" bind:value={visibility}>
			<Select.Trigger class="h-10 w-full text-sm" aria-label="Who can view">
				<span class="flex items-center gap-2">
					<visMeta.icon class="size-4 text-muted-foreground" />
					{visMeta.label}
				</span>
			</Select.Trigger>
			<Select.Content>
				{#each VIS as opt (opt.id)}
					<Select.Item value={opt.id} label={opt.label}>
						<span class="flex items-center gap-2">
							<opt.icon class="size-3.5" />
							{opt.label}
						</span>
					</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
	</section>

	<!-- Options -->
	<section>
		<h3 class="mb-1.5 text-sm font-semibold text-foreground">Options</h3>
		<div
			class="divide-y divide-border-low/50 overflow-hidden rounded-lg border border-border-low/60 bg-background/45"
		>
			<!-- Password -->
			<div class="px-3 py-3">
				<div class="flex items-center gap-2.5">
					<KeyRound class="size-4 shrink-0 text-muted-foreground" />
					<span class="text-sm font-medium text-foreground">Password</span>
				</div>
				{#if hadPassword && !removePassword}
					<div
						class="mt-2 flex items-center justify-between rounded-md border border-border-low/60 px-3 py-2 text-xs"
					>
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
						placeholder={removePassword
							? "Password will be removed"
							: "Set a password (optional)"}
						disabled={removePassword}
						class="mt-2 h-9"
					/>
					{#if removePassword}
						<button
							type="button"
							class="mt-1 text-[11px] font-medium text-muted-foreground hover:underline"
							onclick={() => (removePassword = false)}
						>
							Keep existing password
						</button>
					{/if}
				{/if}
			</div>

			<!-- Expiry -->
			<div class="flex items-center justify-between gap-3 px-3 py-3">
				<div class="flex items-center gap-2.5">
					<CalendarClock class="size-4 shrink-0 text-muted-foreground" />
					<span class="text-sm font-medium text-foreground">Link expiry</span>
				</div>
				<Select.Root type="single" bind:value={expiryChoice}>
					<Select.Trigger class="h-9 w-40 text-sm" aria-label="Link expiry">
						{expiryLabel}
					</Select.Trigger>
					<Select.Content>
						{#if hasExistingExpiry}
							<Select.Item value="keep" label={`Keep until ${initialExpiry}`}>
								Keep until {initialExpiry}
							</Select.Item>
						{/if}
						<Select.Item value="never" label="Never">Never</Select.Item>
						<Select.Item value="7d" label="7 days">7 days</Select.Item>
						<Select.Item value="30d" label="30 days">30 days</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>
		</div>
	</section>
	{/if}
</div>
