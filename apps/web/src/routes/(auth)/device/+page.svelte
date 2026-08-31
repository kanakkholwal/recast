<script lang="ts">
import {
	AlertTriangle,
	ArrowRight,
	Check,
	KeyRound,
	LoaderCircle,
	Monitor,
	X,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { goto, invalidateAll } from "$app/navigation";
import { authClient } from "$lib/auth/client";
import AuthCard from "$lib/auth/components/AuthCard.svelte";
import { formatUserCode, normalizeUserCode } from "./device-code.logic";

let { data } = $props();

// Only used when the desktop didn't pre-fill the code; initialized once, since any `data` change remounts the page.
let manualCode = $state("");
$effect(() => {
	manualCode = data.userCode ?? "";
});
let manualSubmitting = $state(false);

let approving = $state(false);
let denying = $state(false);
const busy = $derived(approving || denying);

async function submitManualCode() {
	const code = normalizeUserCode(manualCode);
	if (!code) return;
	manualSubmitting = true;
	try {
		// Navigating re-runs the load, which redirects unauthenticated users to /login before the session-binding step.
		await goto(`/device?user_code=${encodeURIComponent(code)}`, {
			invalidateAll: true,
		});
	} finally {
		manualSubmitting = false;
	}
}

async function approve() {
	if (busy || !data.userCode) return;
	approving = true;
	const toastId = toast.loading("Approving device…");
	try {
		const { error } = await authClient.device.approve({
			userCode: data.userCode,
		});
		if (error) throw new Error(error.error_description ?? "Couldn't approve the device.");
		toast.success("Device signed in. Return to the desktop app.", {
			id: toastId,
		});
		// The desktop poller picks this up within `interval` seconds; no auto-redirect, since the user came from the desktop.
		await invalidateAll();
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't approve the device.", {
			id: toastId,
		});
		approving = false;
	}
}

async function deny() {
	if (busy || !data.userCode) return;
	denying = true;
	const userCode = data.userCode;
	try {
		await toast.promise(
			(async () => {
				const { error } = await authClient.device.deny({
					userCode,
				});
				if (error) throw new Error(error.error_description ?? "Couldn't deny the request.");
			})(),
			{
				loading: "Denying…",
				success: "Device request denied.",
				error: (err) => (err as Error)?.message ?? "Couldn't deny the request.",
			},
		);
		await invalidateAll();
	} finally {
		denying = false;
	}
}

// 'pending' waits on approval, 'approved' follows the Approve click via invalidateAll, and 'denied' is a rejection.
const deviceStatus = $derived((data.device as { status?: string } | null)?.status ?? null);

const deviceTitle = $derived(
	!data.userCode
		? "Enter your device code"
		: data.error
			? "Code not recognized"
			: deviceStatus === "approved"
				? "You're all set"
				: deviceStatus === "denied"
					? "Sign-in denied"
					: "Sign in to Recast Desktop?",
);

const deviceBody = $derived(
	!data.userCode
		? "Type the code shown in your Recast Desktop app."
		: data.error
			? data.error
			: deviceStatus === "approved"
				? "Your Recast Desktop is signed in. Hop back over, cloud sync is ready."
				: deviceStatus === "denied"
					? "The desktop request was rejected. Start a new sign-in from the app if this was a mistake."
					: "Approving links this account to the desktop so it can sync your recordings.",
);
</script>

<svelte:head>
	<title>Authorize device - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<!--
	Outer wrapper (grid centering + gradient + back-to-site link) is provided
	by `(auth)/+layout.svelte`. Don't re-add it here — the root layout already
	excludes `/device` from the marketing chrome via the chromelessPaths set.
-->
<AuthCard eyebrowIcon={Monitor} eyebrow="Authorize device" title={deviceTitle} description={deviceBody}>
			{#if !data.userCode}
				<!-- Manual code entry. We don't require sign-in to render this —
				     the user might be writing the code down before they sign in.
				     The /device?user_code=... navigation triggers +page.server.ts
				     which redirects unauthenticated users through /login. -->
				<form
					onsubmit={(e) => {
						e.preventDefault();
						submitManualCode();
					}}
					class="flex flex-col gap-3"
				>
					<label
						for="device-code-input"
						class="text-caption font-medium text-muted-foreground"
					>
						Device code
					</label>
					<div class="relative">
						<KeyRound
							class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
						/>
						<input
							id="device-code-input"
							type="text"
							bind:value={manualCode}
							placeholder="ABCD-1234"
							autocomplete="off"
							spellcheck="false"
							maxlength="12"
							class="h-11 w-full rounded-lg border border-border bg-background pl-9 pr-3 font-mono text-body font-medium text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary"
						/>
					</div>
					<Button
						variant="dark"
						type="submit"
						disabled={manualSubmitting || manualCode.trim().length === 0}
						class="group/cta w-full gap-2"
					>
						{#if manualSubmitting}
							<LoaderCircle class="size-4 animate-spin" />
						{/if}
						{manualSubmitting ? "Checking…" : "Continue"}
						{#if !manualSubmitting}
							<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
						{/if}
					</Button>
				</form>
			{:else if data.error}
				<div class="flex flex-col gap-3 text-body-sm text-muted-foreground">
					<AlertTriangle class="size-5 text-tag-tangerine" />
					<span>{data.error}</span>
					<Button href="/device" variant="outline" size="sm" class="mt-2">
						Enter a different code
					</Button>
				</div>
			{:else if deviceStatus === "approved"}
				<div
					class="flex flex-col gap-5"
					in:fly={{ y: 8, duration: 360, easing: cubicOut }}
				>
					<Check class="size-10 text-tag-green" stroke={1.75} />
					<div class="flex flex-col gap-1.5">
						<p class="text-body font-semibold text-foreground">
							Desktop signed in
						</p>
						<p class="text-caption leading-relaxed text-muted-foreground">
							{#if data.viewer?.email}
								Linked to <span class="font-medium text-foreground">{data.viewer.email}</span>.
							{/if}
							You can close this tab.
						</p>
					</div>
					<div class="flex w-full flex-col gap-2 pt-1">
						<Button href="/dashboard" variant="outline" size="sm" class="w-full gap-1.5">
							<ArrowRight class="size-3.5" />
							<span>Go to dashboard</span>
						</Button>
					</div>
				</div>
			{:else if deviceStatus === "denied"}
				<div
					class="flex flex-col gap-5"
					in:fly={{ y: 8, duration: 360, easing: cubicOut }}
				>
					<X class="size-10 text-destructive" stroke={1.75} />
					<div class="flex flex-col gap-1.5">
						<p class="text-body font-semibold text-foreground">
							Sign-in denied
						</p>
						<p class="text-caption leading-relaxed text-muted-foreground">
							The desktop won't be signed in. Start a new sign-in from your Recast Desktop app if this was a mistake.
						</p>
					</div>
				</div>
			{:else}
				<!-- Authenticated + bound. Show approval card with the code for
				     visual confirmation against the desktop screen. -->
				<div class="flex flex-col gap-4">
					<div
						class="rounded-xl border border-border-low bg-background/50 p-4 text-center"
					>
						<div class="text-caption font-medium text-muted-foreground">
							Code
						</div>
						<div
							class="mt-1.5 font-mono text-heading-sm font-semibold tracking-[0.3em] text-foreground"
						>
							{formatUserCode(data.userCode)}
						</div>
					</div>
					<p class="text-caption text-muted-foreground">
						Make sure this matches the code shown in your Recast Desktop app
						before approving.
					</p>
					<div class="flex flex-col gap-2.5">
						<Button onclick={approve} disabled={busy} variant="dark" class="group/cta w-full gap-2">
							{#if approving}
								<LoaderCircle class="size-4 animate-spin" />
							{:else}
								<Check class="size-4" />
							{/if}
							{approving ? "Approving…" : "Approve & sign in desktop"}
						</Button>
						<Button
							variant="ghost"
							onclick={deny}
							disabled={busy}
							class="w-full gap-2 text-muted-foreground"
						>
							{#if denying}
								<LoaderCircle class="size-4 animate-spin" />
							{:else}
								<X class="size-4" />
							{/if}
							{denying ? "Denying…" : "Deny"}
						</Button>
					</div>
					<p class="text-caption text-muted-foreground">
						Signed in as <span class="font-medium text-foreground">{data.viewer?.email ?? ""}</span>
					</p>
				</div>
			{/if}
</AuthCard>
