<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import { quickUpload } from "$lib/dashboard/quick-upload.svelte";
	import {
	  createRecastShare,
	  isUploadableVideo,
	  uploadRecastFile,
	  UPLOAD_ACCEPT,
	  type ShareOptions,
	  type ShareVisibility,
	  type UploadPhase,
	} from "$lib/dashboard/upload";
	import {
	  Building2,
	  CalendarClock,
	  Check,
	  CheckCircle2,
	  Copy,
	  FileVideo,
	  Globe2,
	  KeyRound,
	  Link2,
	  LoaderCircle,
	  Lock,
	  RotateCcw,
	  UploadCloud,
	  Users,
	} from "@lucide/svelte";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import * as Select from "@recast/ui/select";
	import { toast } from "@recast/ui/sonner";
	import { Textarea } from "@recast/ui/textarea";
	import { cn } from "@recast/ui/utils";

	let {
		workspaceId,
		workspaceName,
		plan,
	}: {
		workspaceId: string | undefined;
		workspaceName?: string | null;
		plan?: string | null;
	} = $props();

	// The flow: pick a file → watch it upload → choose sharing → get the link.
	// Sharing settings live AFTER the upload, so nothing gates the drop.
	type Step = "pick" | "uploading" | "configure" | "done";
	let step = $state<Step>("pick");

	let fileInput = $state<HTMLInputElement | null>(null);
	let isDragging = $state(false);

	// Upload progress state.
	let phase = $state<UploadPhase>("preparing");
	let pct = $state(0);
	let fileName = $state("");
	let recastId = $state<string | null>(null);

	// Sharing state (used only once the upload has finished).
	let creatingLink = $state(false);
	let result = $state<{ slug: string; shareUrl: string } | null>(null);
	let visibility = $state<ShareVisibility>("workspace");
	let inviteesRaw = $state("");
	let commentsEnabled = $state(true);
	let passwordEnabled = $state(false);
	let password = $state("");
	let expiry = $state<"never" | "7d" | "30d">("never");

	const isPro = $derived(plan === "pro");

	const visibilityLabel = $derived.by(() => {
		switch (visibility) {
			case "public":
				return "Anyone with the link";
			case "workspace":
				return `${workspaceName || "Workspace"} members`;
			case "selected":
				return "Selected people";
			case "private":
				return "Only workspace admins";
		}
	});
	const parsedInvitees = $derived.by(() =>
		inviteesRaw
			.split(/[\n,]/)
			.map((email) => email.trim().toLowerCase())
			.filter(Boolean)
			.map((email) => ({ email, role: "viewer" as const })),
	);
	const shareOptions = $derived.by<ShareOptions>(() => ({
		visibility,
		commentsEnabled,
		...(visibility === "selected" ? { invitees: parsedInvitees } : {}),
		...(isPro && passwordEnabled && password.trim() ? { password: password.trim() } : {}),
		...(isPro && expiry !== "never" ? { expiresAt: expiresAt(expiry) } : {}),
	}));

	// Journey framing — a lightweight 3-stage indicator for the sense of
	// progress ("almost there") across the whole flow.
	const STAGES = ["Upload", "Settings", "Share"];
	const stageIndex = $derived(
		step === "pick" || step === "uploading" ? 0 : step === "configure" ? 1 : 2,
	);

	// Per-phase upload steps + an overall percentage that eases across them.
	const UPLOAD_STEPS = [
		{ key: "preparing", label: "Preparing file" },
		{ key: "uploading", label: "Uploading video" },
		{ key: "finalizing", label: "Finalizing" },
	] as const;
	const PHASE_ORDER: Record<UploadPhase, number> = {
		preparing: 0,
		uploading: 1,
		finalizing: 2,
		sharing: 3,
	};
	const currentPhaseIndex = $derived(PHASE_ORDER[phase]);
	const overallPct = $derived.by(() => {
		if (phase === "preparing") return 6;
		if (phase === "uploading") return 8 + Math.round(pct * 0.82);
		return 96;
	});
	function stepStatus(i: number): "done" | "active" | "pending" {
		if (i < currentPhaseIndex) return "done";
		if (i === currentPhaseIndex) return "active";
		return "pending";
	}

	function expiresAt(value: "never" | "7d" | "30d"): string | null {
		if (value === "never") return null;
		const days = value === "7d" ? 7 : 30;
		return new Date(Date.now() + days * 24 * 60 * 60 * 1000).toISOString();
	}

	function reset() {
		step = "pick";
		phase = "preparing";
		pct = 0;
		fileName = "";
		recastId = null;
		result = null;
		creatingLink = false;
		isDragging = false;
		visibility = "workspace";
		inviteesRaw = "";
		commentsEnabled = true;
		passwordEnabled = false;
		password = "";
		expiry = "never";
	}

	function close() {
		if (step === "uploading") return; // don't abandon an in-flight upload
		quickUpload.hide();
		reset();
	}

	async function startUpload(file: File | undefined) {
		if (!file || step === "uploading") return;
		if (!isUploadableVideo(file)) {
			toast.error("Only .mp4 video files can be uploaded here.");
			return;
		}
		step = "uploading";
		phase = "preparing";
		pct = 0;
		fileName = file.name;
		result = null;
		try {
			const r = await uploadRecastFile(file, {
				workspaceId,
				autoShare: false,
				onPhase: (p) => (phase = p),
				onProgress: (v) => (pct = v),
			});
			recastId = r.recastId;
			// The recast is published (unshared) — surface it in the library now.
			void invalidateAll();
			step = "configure";
		} catch (err) {
			toast.error((err as Error)?.message ?? "Couldn't upload that file.");
			step = "pick";
		}
	}

	function validateSettings(): boolean {
		if (visibility === "selected" && parsedInvitees.length === 0) {
			toast.error("Add at least one email for selected sharing.");
			return false;
		}
		if (isPro && passwordEnabled && password.trim().length > 0 && password.trim().length < 4) {
			toast.error("Password must be at least 4 characters.");
			return false;
		}
		return true;
	}

	async function createLink() {
		if (!recastId || creatingLink) return;
		if (!validateSettings()) return;
		creatingLink = true;
		try {
			const r = await createRecastShare(recastId, shareOptions);
			result = r;
			void invalidateAll();
			try {
				await navigator.clipboard.writeText(r.shareUrl);
				toast.success("Share link created and copied.");
			} catch {
				toast.success("Share link created.");
			}
			step = "done";
		} catch (err) {
			toast.error((err as Error)?.message ?? "Couldn't create the share link.");
		} finally {
			creatingLink = false;
		}
	}

	function onFilePicked(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = "";
		startUpload(file);
	}
	function onDragOver(e: DragEvent) {
		e.preventDefault();
		if (step === "pick") isDragging = true;
	}
	function onDragLeave(e: DragEvent) {
		if (e.currentTarget === e.target) isDragging = false;
	}
	function onDrop(e: DragEvent) {
		e.preventDefault();
		isDragging = false;
		startUpload(e.dataTransfer?.files?.[0]);
	}

	async function copyLink() {
		if (!result) return;
		try {
			await navigator.clipboard.writeText(result.shareUrl);
			toast.success("Share link copied.");
		} catch {
			toast.error("Couldn't copy the link.");
		}
	}

	const stepTitle = $derived(
		step === "done" ? "Recast shared" : step === "configure" ? "Share your recast" : "New recast",
	);
	const stepDescription = $derived.by(() => {
		switch (step) {
			case "uploading":
				return "Hang tight — we're uploading your recast.";
			case "configure":
				return "Choose who can see it, then create the link.";
			case "done":
				return "Your share link is ready to send.";
			default:
				return `Upload an MP4 to ${workspaceName || "the current workspace"}.`;
		}
	});
</script>

<Dialog.Root
	bind:open={quickUpload.open}
	onOpenChange={(open) => {
		if (open) return;
		// Block dismissal mid-upload; otherwise reset + close.
		if (step === "uploading") {
			quickUpload.open = true;
			return;
		}
		close();
	}}
>
	<Dialog.Content
		showCloseButton={step !== "uploading"}
		class="max-h-[min(92vh,720px)] gap-0 overflow-hidden p-0 sm:max-w-lg"
	>
		<Dialog.Header class="border-b border-border/60 px-5 py-4 pr-12">
			<Dialog.Title>{stepTitle}</Dialog.Title>
			<Dialog.Description>{stepDescription}</Dialog.Description>

			<!-- Journey stages -->
			<div class="mt-3 flex items-center gap-2" aria-hidden="true">
				{#each STAGES as label, i (label)}
					<div class="flex flex-1 flex-col gap-1">
						<div
							class={cn(
								"h-1 rounded-full transition-colors duration-300",
								i < stageIndex ? "bg-primary" : i === stageIndex ? "bg-primary/60" : "bg-foreground/10",
							)}
						></div>
						<span
							class={cn(
								"text-[10px] font-medium uppercase tracking-[0.1em] transition-colors",
								i <= stageIndex ? "text-foreground/70" : "text-muted-foreground/50",
							)}
						>
							{label}
						</span>
					</div>
				{/each}
			</div>
		</Dialog.Header>

		<div class="overflow-y-auto px-5 py-5">
			{#if step === "pick"}
				<input
					bind:this={fileInput}
					type="file"
					accept={UPLOAD_ACCEPT}
					class="hidden"
					onchange={onFilePicked}
				/>
				<button
					type="button"
					onclick={() => fileInput?.click()}
					ondragover={onDragOver}
					ondragleave={onDragLeave}
					ondrop={onDrop}
					class={cn(
						"flex min-h-64 w-full flex-col items-center justify-center rounded-xl border border-dashed px-6 py-10 text-center outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/45",
						isDragging
							? "border-primary/55 bg-primary/8"
							: "border-border-low/70 bg-background/45 hover:border-primary/35 hover:bg-background/70",
					)}
				>
					<span class="glass-chip grid size-14 place-items-center rounded-2xl text-primary">
						<UploadCloud class="size-6" />
					</span>
					<span class="mt-4 text-base font-semibold text-foreground">Drop MP4 or browse</span>
					<span class="mt-1 text-sm text-muted-foreground">
						Sharing options come after the upload.
					</span>
					<span class="mt-4 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60">
						MP4 only
					</span>
				</button>
			{:else if step === "uploading"}
				<div class="mx-auto max-w-sm py-2">
					<div class="flex items-center gap-2 text-sm">
						<FileVideo class="size-4 shrink-0 text-muted-foreground" />
						<span class="min-w-0 truncate font-medium text-foreground">{fileName}</span>
					</div>

					<div class="mt-4 h-2 overflow-hidden rounded-full bg-foreground/10">
						<div
							class="h-full rounded-full bg-linear-to-r from-primary/70 to-primary transition-[width] duration-300 ease-out"
							style="width: {overallPct}%"
						></div>
					</div>
					<div class="mt-1.5 text-right text-xs font-medium tabular-nums text-muted-foreground">
						{overallPct}%
					</div>

					<ol class="mt-6 space-y-3.5">
						{#each UPLOAD_STEPS as s, i (s.key)}
							{@const st = stepStatus(i)}
							<li class="flex items-center gap-3">
								<span
									class={cn(
										"grid size-6 shrink-0 place-items-center rounded-full transition-colors",
										st === "done"
											? "bg-primary/12 text-primary"
											: st === "active"
												? "bg-primary/12 text-primary"
												: "bg-foreground/6 text-muted-foreground/50",
									)}
								>
									{#if st === "done"}
										<Check class="size-3.5" />
									{:else if st === "active"}
										<LoaderCircle class="size-3.5 animate-spin" />
									{:else}
										<span class="size-1.5 rounded-full bg-current"></span>
									{/if}
								</span>
								<span
									class={cn(
										"text-sm transition-colors",
										st === "pending" ? "text-muted-foreground/60" : "font-medium text-foreground",
									)}
								>
									{s.label}
									{#if st === "active" && s.key === "uploading"}
										<span class="ml-1 tabular-nums text-muted-foreground">{pct}%</span>
									{/if}
								</span>
							</li>
						{/each}
					</ol>
				</div>
			{:else if step === "configure"}
				<div class="space-y-4">
					<div class="flex items-center gap-2.5 rounded-lg border border-success/25 bg-success/8 px-3 py-2.5">
						<CheckCircle2 class="size-4 shrink-0 text-success" />
						<span class="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
							{fileName}
						</span>
						<span class="shrink-0 text-xs font-medium text-success">Uploaded</span>
					</div>

					<section>
						<h3 class="mb-2 text-sm font-semibold text-foreground">Who can see it</h3>
						<Select.Root type="single" bind:value={visibility}>
							<Select.Trigger class="h-10 w-full text-sm" aria-label="Share visibility">
								{visibilityLabel}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="workspace">
									<span class="flex items-center gap-2"><Building2 class="size-3.5" /> Workspace members</span>
								</Select.Item>
								<Select.Item value="public">
									<span class="flex items-center gap-2"><Globe2 class="size-3.5" /> Anyone with the link</span>
								</Select.Item>
								<Select.Item value="selected">
									<span class="flex items-center gap-2"><Users class="size-3.5" /> Selected people</span>
								</Select.Item>
								<Select.Item value="private">
									<span class="flex items-center gap-2"><Lock class="size-3.5" /> Private</span>
								</Select.Item>
							</Select.Content>
						</Select.Root>

						{#if visibility === "selected"}
							<Label class="mt-2.5 block">
								<span class="mb-1 block text-xs font-semibold text-foreground/85">People</span>
								<Textarea
									bind:value={inviteesRaw}
									placeholder="alex@company.com, sam@company.com"
									class="min-h-20 resize-none text-sm"
								/>
								<span class="mt-1 block text-[11px] text-muted-foreground">
									Separate emails with commas or new lines.
								</span>
							</Label>
						{/if}
					</section>

					<section class="space-y-2.5">
						<button
							type="button"
							role="switch"
							aria-checked={commentsEnabled}
							onclick={() => (commentsEnabled = !commentsEnabled)}
							class="flex min-h-11 w-full items-center justify-between gap-3 rounded-lg border border-border-low/60 bg-background/45 px-3 text-left transition-colors hover:bg-background/70"
						>
							<span>
								<span class="block text-sm font-medium text-foreground">Allow viewer comments</span>
								<span class="block text-xs text-muted-foreground">Reactions stay available either way.</span>
							</span>
							<span class={cn("h-5 w-9 shrink-0 rounded-full p-0.5 transition-colors", commentsEnabled ? "bg-primary" : "bg-foreground/15")}>
								<span class={cn("block size-4 rounded-full bg-background transition-transform", commentsEnabled && "translate-x-4")}></span>
							</span>
						</button>

						<div class="rounded-lg border border-border-low/60 bg-background/45 p-3">
							<div class="flex items-center justify-between gap-3">
								<div class="flex items-center gap-2">
									<KeyRound class="size-4 text-muted-foreground" />
									<span class="text-sm font-medium text-foreground">Password protection</span>
								</div>
								{#if !isPro}<Badge variant="outline">Pro</Badge>{/if}
							</div>
							{#if isPro}
								<div class="mt-3 grid gap-2 sm:grid-cols-[auto_1fr] sm:items-center">
									<button
										type="button"
										role="switch"
										aria-label="Enable password protection"
										aria-checked={passwordEnabled}
										onclick={() => (passwordEnabled = !passwordEnabled)}
										class={cn("h-5 w-9 rounded-full p-0.5 transition-colors", passwordEnabled ? "bg-primary" : "bg-foreground/15")}
									>
										<span class={cn("block size-4 rounded-full bg-background transition-transform", passwordEnabled && "translate-x-4")}></span>
									</button>
									<Input
										bind:value={password}
										type="password"
										disabled={!passwordEnabled}
										placeholder="Optional password"
										class="h-9"
									/>
								</div>
							{:else}
								<p class="mt-2 text-xs text-muted-foreground">Upgrade to add a password on shared recasts.</p>
							{/if}
						</div>

						<div class="rounded-lg border border-border-low/60 bg-background/45 p-3">
							<div class="mb-2 flex items-center justify-between gap-3">
								<div class="flex items-center gap-2">
									<CalendarClock class="size-4 text-muted-foreground" />
									<span class="text-sm font-medium text-foreground">Link expiry</span>
								</div>
								{#if !isPro}<Badge variant="outline">Pro</Badge>{/if}
							</div>
							{#if isPro}
								<Select.Root type="single" bind:value={expiry}>
									<Select.Trigger class="h-9 w-full text-sm" aria-label="Link expiry">
										{expiry === "never" ? "Never expires" : expiry === "7d" ? "Expires in 7 days" : "Expires in 30 days"}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="never">Never expires</Select.Item>
										<Select.Item value="7d">7 days</Select.Item>
										<Select.Item value="30d">30 days</Select.Item>
									</Select.Content>
								</Select.Root>
							{:else}
								<div class="flex h-9 items-center rounded-md border border-border-low/60 bg-muted/30 px-3 text-sm text-muted-foreground">
									Expires in 15 days
								</div>
								<p class="mt-1.5 text-[11px] text-muted-foreground">
									Free links expire after 15 days. Upgrade to keep them longer.
								</p>
							{/if}
						</div>
					</section>

					<Button class="h-10 w-full gap-2" disabled={creatingLink} onclick={createLink}>
						{#if creatingLink}
							<LoaderCircle class="size-4 animate-spin" />
							Creating link…
						{:else}
							<Link2 class="size-4" />
							Create share link
						{/if}
					</Button>
				</div>
			{:else if step === "done" && result}
				<div class="py-2 text-center">
					<span class="mx-auto grid size-14 place-items-center rounded-2xl bg-success/12 text-success">
						<CheckCircle2 class="size-7" />
					</span>
					<h3 class="mt-4 text-base font-semibold text-foreground">Your recast is shared</h3>
					<p class="mt-1 text-sm text-muted-foreground">Anyone you send this link to can watch it.</p>

					<div class="mt-5 flex min-w-0 items-center gap-2 rounded-lg border border-border-low/60 bg-muted/30 px-3 py-2.5 text-left">
						<Link2 class="size-3.5 shrink-0 text-muted-foreground" />
						<a
							href={result.shareUrl}
							target="_blank"
							rel="noreferrer"
							class="min-w-0 flex-1 truncate text-xs font-medium text-foreground hover:text-primary"
						>
							{result.shareUrl}
						</a>
					</div>

					<div class="mt-3 grid grid-cols-2 gap-2">
						<Button variant="outline" class="gap-2" onclick={copyLink}>
							<Copy class="size-4" />
							Copy link
						</Button>
						<Button href={result.shareUrl} target="_blank" class="gap-2">
							<Link2 class="size-4" />
							Open
						</Button>
					</div>

					<button
						type="button"
						onclick={reset}
						class="mx-auto mt-4 inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
					>
						<RotateCcw class="size-3.5" />
						Upload another
					</button>
				</div>
			{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
