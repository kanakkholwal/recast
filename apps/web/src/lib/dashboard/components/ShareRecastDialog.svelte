<script lang="ts">
import {
	Building2,
	CalendarClock,
	CheckCircle2,
	Copy,
	ExternalLink,
	Globe2,
	KeyRound,
	Link2,
	LoaderCircle,
	Lock,
	Users,
} from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import * as Dialog from "@recast/ui/dialog";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import * as Select from "@recast/ui/select";
import { toast } from "@recast/ui/sonner";
import { Switch } from "@recast/ui/switch";
import { Textarea } from "@recast/ui/textarea";
import { browser } from "$app/environment";
import { invalidateAll } from "$app/navigation";
import { shareDialog } from "$lib/dashboard/share-dialog.svelte";
import { quotaStore } from "$lib/dashboard/store.svelte";
import { createRecastShare, type ShareOptions, type ShareVisibility } from "$lib/dashboard/upload";

const isPro = $derived(quotaStore.value?.plan === "pro" || quotaStore.value?.plan === "enterprise");

let creating = $state(false);
// The just-created link — switches the dialog into its "ready" state.
let created = $state<{ slug: string; shareUrl: string } | null>(null);

let visibility = $state<ShareVisibility>("public");
let inviteesRaw = $state("");
let commentsEnabled = $state(true);
let passwordEnabled = $state(false);
let password = $state("");
let expiry = $state<"never" | "7d" | "30d">("never");

function resetForm() {
	visibility = "public";
	inviteesRaw = "";
	commentsEnabled = true;
	passwordEnabled = false;
	password = "";
	expiry = "never";
}

// Fresh form + no stale success view each time the dialog opens.
$effect(() => {
	if (shareDialog.open) {
		created = null;
		resetForm();
	}
});

const visibilityLabel = $derived.by(() => {
	switch (visibility) {
		case "public":
			return "Anyone with the link";
		case "workspace":
			return "Workspace members";
		case "selected":
			return "Specific people";
		case "private":
			return "Only workspace admins";
	}
});
const VisibilityIcon = $derived(
	visibility === "public"
		? Globe2
		: visibility === "selected"
			? Users
			: visibility === "private"
				? Lock
				: Building2,
);

const parsedInvitees = $derived(
	inviteesRaw
		.split(/[\n,]/)
		.map((e) => e.trim().toLowerCase())
		.filter(Boolean)
		.map((email) => ({ email, role: "viewer" as const })),
);

function expiresAtIso(value: "never" | "7d" | "30d"): string | null {
	if (value === "never") return null;
	const days = value === "7d" ? 7 : 30;
	return new Date(Date.now() + days * 86_400_000).toISOString();
}

function validate(): boolean {
	if (visibility === "selected" && parsedInvitees.length === 0) {
		toast.error("Add at least one email for specific-people sharing.");
		return false;
	}
	if (isPro && passwordEnabled && password.trim().length > 0 && password.trim().length < 4) {
		toast.error("Password must be at least 4 characters.");
		return false;
	}
	return true;
}

async function create() {
	const recastId = shareDialog.recastId;
	if (!recastId || creating || !validate()) return;
	creating = true;
	try {
		const options: ShareOptions = {
			visibility,
			commentsEnabled,
			...(visibility === "selected" ? { invitees: parsedInvitees } : {}),
			...(isPro && passwordEnabled && password.trim() ? { password: password.trim() } : {}),
			...(isPro && expiry !== "never" ? { expiresAt: expiresAtIso(expiry) } : {}),
		};
		const result = await createRecastShare(recastId, options);
		await invalidateAll();
		// Show the link right here instead of closing — the user copies it deliberately.
		created = result;
	} catch (e) {
		toast.error((e as Error)?.message ?? "Couldn't create the share link.");
	} finally {
		creating = false;
	}
}

function createAnother() {
	created = null;
	resetForm();
}

async function copyUrl(url: string) {
	if (!browser) return;
	try {
		await navigator.clipboard.writeText(url);
		toast.success("Share link copied to clipboard.");
	} catch {
		toast.error("Couldn't copy the link.");
	}
}
</script>

<Dialog.Root
	bind:open={shareDialog.open}
	onOpenChange={(open) => {
		if (!open) shareDialog.hide();
	}}
>
	<Dialog.Content class="gap-0 overflow-hidden p-0 sm:max-w-lg">
		<Dialog.Header class="border-b border-border-low px-5 py-4 pr-12 font-display font-medium">
			<Dialog.Title>{created ? "Link ready" : "Share this recast"}</Dialog.Title>
			<Dialog.Description>
				{created ? "Copy it and send it to whoever you want." : "Choose who can see it, then create the link."}
			</Dialog.Description>
		</Dialog.Header>

		{#if created}
			<!-- Ready: show the fresh link -->
			<div class="space-y-4 px-5 py-6">
				<div class="flex flex-col items-center text-center">
					<CheckCircle2 class="size-6 text-success" />
					<p class="mt-3 font-display text-body font-medium text-foreground">
						Your share link is ready
					</p>
				</div>

				<div class="flex items-center gap-2 rounded-lg border border-border-low bg-paper px-3 py-2.5">
					<Link2 class="size-3.5 shrink-0 text-muted-foreground" />
					<span class="min-w-0 flex-1 truncate text-body-sm text-foreground">{created.shareUrl}</span>
				</div>

				<div class="grid grid-cols-2 gap-2">
					<Button variant="outline" class="gap-2" onclick={() => copyUrl(created!.shareUrl)}>
						<Copy class="size-4" /> Copy link
					</Button>
					<Button href={created.shareUrl} target="_blank" variant="dark" class="gap-2">
						<ExternalLink class="size-4" /> Open
					</Button>
				</div>
			</div>

			<div class="flex items-center justify-between gap-2 border-t border-border-low px-5 py-4">
				<button
					type="button"
					onclick={createAnother}
					class="text-body-sm font-medium text-foreground outline-none hover:underline focus-visible:underline"
				>
					Create another link
				</button>
				<Button variant="outline" size="sm" onclick={() => shareDialog.hide()}>Done</Button>
			</div>
		{:else}
			<div class="max-h-[min(80vh,560px)] space-y-5 overflow-y-auto px-5 py-5">
				<!-- Access control -->
				<section>
					<h3 class="mb-1.5 text-body-sm font-medium text-foreground">Who can see it</h3>
					<Select.Root type="single" bind:value={visibility}>
						<Select.Trigger class="h-10 w-full text-body-sm" aria-label="Share visibility">
							<span class="flex items-center gap-2">
								<VisibilityIcon class="size-4 text-muted-foreground" />
								{visibilityLabel}
							</span>
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="public">
								<span class="flex items-center gap-2"><Globe2 class="size-3.5" /> Anyone with the link</span>
							</Select.Item>
							<Select.Item value="workspace">
								<span class="flex items-center gap-2"><Building2 class="size-3.5" /> Workspace members</span>
							</Select.Item>
							<Select.Item value="selected">
								<span class="flex items-center gap-2"><Users class="size-3.5" /> Specific people</span>
							</Select.Item>
							<Select.Item value="private">
								<span class="flex items-center gap-2"><Lock class="size-3.5" /> Only workspace admins</span>
							</Select.Item>
						</Select.Content>
					</Select.Root>

					{#if visibility === "selected"}
						<Label class="mt-2.5 block">
							<span class="mb-1.5 block text-body-sm font-medium text-foreground">People</span>
							<Textarea
								bind:value={inviteesRaw}
								placeholder="alex@company.com, sam@company.com"
								class="min-h-20 resize-none text-body-sm"
							/>
							<span class="mt-1.5 block text-caption text-muted-foreground">
								Separate emails with commas or new lines.
							</span>
						</Label>
					{/if}
				</section>

				<!-- Options -->
				<section>
					<h3 class="mb-1.5 text-body-sm font-medium text-foreground">Options</h3>
					<div class="divide-y divide-border-low overflow-hidden rounded-lg border border-border-low bg-paper">
						<div class="flex items-center justify-between gap-3 px-3 py-3">
							<span class="text-body-sm font-medium text-foreground">Allow viewer comments</span>
							<Switch bind:checked={commentsEnabled} aria-label="Allow viewer comments" />
						</div>

						<div class="px-3 py-3">
							<div class="flex items-center justify-between gap-3">
								<div class="flex min-w-0 items-center gap-2.5">
									<KeyRound class="size-4 shrink-0 text-muted-foreground" />
									<span class="text-body-sm font-medium text-foreground">Password</span>
									{#if !isPro}<Badge variant="outline">Pro</Badge>{/if}
								</div>
								{#if isPro}
									<Switch bind:checked={passwordEnabled} aria-label="Require a password" />
								{/if}
							</div>
							{#if isPro && passwordEnabled}
								<Input bind:value={password} type="password" placeholder="Set a password" class="mt-2.5 h-9 border-border-low bg-background" />
							{:else if !isPro}
								<p class="mt-1.5 text-caption text-muted-foreground">Protect links with a password on Pro.</p>
							{/if}
						</div>

						<div class="flex items-center justify-between gap-3 px-3 py-3">
							<div class="flex min-w-0 items-center gap-2.5">
								<CalendarClock class="size-4 shrink-0 text-muted-foreground" />
								<span class="text-body-sm font-medium text-foreground">Link expiry</span>
								{#if !isPro}<Badge variant="outline">Pro</Badge>{/if}
							</div>
							{#if isPro}
								<Select.Root type="single" bind:value={expiry}>
									<Select.Trigger class="h-9 w-36 text-body-sm" aria-label="Link expiry">
										{expiry === "never" ? "Never" : expiry === "7d" ? "7 days" : "30 days"}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="never">Never expires</Select.Item>
										<Select.Item value="7d">7 days</Select.Item>
										<Select.Item value="30d">30 days</Select.Item>
									</Select.Content>
								</Select.Root>
							{:else}
								<span class="shrink-0 text-body-sm text-muted-foreground">15 days</span>
							{/if}
						</div>
					</div>
					{#if !isPro}
						<p class="mt-1.5 text-caption text-muted-foreground">Free links expire after 15 days.</p>
					{/if}
				</section>
			</div>

			<div class="border-t border-border-low px-5 py-4">
				<Button variant="dark" class="h-10 w-full gap-2" disabled={creating} onclick={create}>
					{#if creating}
						<LoaderCircle class="size-4 animate-spin" />
						Creating link…
					{:else}
						<Link2 class="size-4" />
						Create link
					{/if}
				</Button>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
