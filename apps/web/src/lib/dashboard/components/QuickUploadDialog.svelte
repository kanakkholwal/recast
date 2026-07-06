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
	  MessageSquare,
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
		firstUpload = false,
	}: {
		workspaceId: string | undefined;
		workspaceName?: string | null;
		plan?: string | null;
		/** No recasts yet, show the endowed-progress (account already done) journey. */
		firstUpload?: boolean;
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
	const VisibilityIcon = $derived(
		visibility === "public"
			? Globe2
			: visibility === "selected"
				? Users
				: visibility === "private"
					? Lock
					: Building2,
	);
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

	// Journey framing, a lightweight stage indicator for the sense of progress
	// across the whole flow. First-time users get an endowed-progress variant:
	// the account they already created counts as a completed first step, so the
	// bar reads "1 of 4 done" rather than "0 of 3", a goal-gradient nudge that
	// lifts follow-through. Latched when the journey begins so a mid-flow quota
	// refresh (post-upload invalidate) can't drop a segment underfoot.
	let endowed = $state(false);
	$effect(() => {
		if (quickUpload.open && step === "pick") endowed = firstUpload;
	});
	const stages = $derived(
		endowed ? ["Account", "Upload", "Settings", "Share"] : ["Upload", "Settings", "Share"],
	);
	const baseStageIndex = $derived(
		step === "pick" || step === "uploading" ? 0 : step === "configure" ? 1 : 2,
	);
	const stageIndex = $derived(endowed ? baseStageIndex + 1 : baseStageIndex);

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
			toast.error("Only MP4 or WebM videos can be uploaded here.");
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
			// The recast is published (unshared), surface it in the library now.
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

	// A file dropped elsewhere (e.g. the library) opens the dialog with the file
	// staged, start its upload straight away so the drop and the button share
	// one flow. Guarded on the `pick` step so it never re-fires mid-journey.
	$effect(() => {
		if (quickUpload.open && quickUpload.pendingFile && step === "pick") {
			const file = quickUpload.pendingFile;
			quickUpload.pendingFile = null;
			startUpload(file);
		}
	});

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
				return "Hang tight, we're uploading your recast.";
			case "configure":
				return "Choose who can see it, then create the link.";
			case "done":
				return "Your share link is ready to send.";
			default:
				return `Upload an MP4 or WebM to ${workspaceName || "your workspace"}.`;
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
				{#each stages as label, i (label)}
					{@const done = i < stageIndex}
					{@const active = i === stageIndex}
					<div class="flex flex-1 flex-col gap-1">
						<div
							class={cn(
								"h-1 rounded-full transition-colors duration-300",
								done ? "bg-primary" : active ? "bg-primary/60" : "bg-foreground/10",
							)}
						></div>
						<span
							class={cn(
								"flex items-center gap-1 text-[10px] font-medium uppercase tracking-widest transition-colors",
								done || active ? "text-foreground/70" : "text-muted-foreground/50",
							)}
						>
							{#if done}<Check class="size-2.5 shrink-0 text-primary" />{/if}
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
					<span class="mt-4 text-base font-semibold text-foreground">Drop a video or browse</span>
					<span class="mt-1 text-sm text-muted-foreground">
						Set visibility, comments, and expiry after upload.
					</span>
					<span class="mt-4 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60">
						MP4 · WebM
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
				<div class="space-y-5">
					<!-- Upload confirmation -->
					<div class="flex items-center gap-2.5 rounded-lg border border-success/25 bg-success/8 px-3 py-2.5">
						<CheckCircle2 class="size-4 shrink-0 text-success" />
						<span class="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
							{fileName}
						</span>
						<span class="shrink-0 text-xs font-medium text-success">Uploaded</span>
					</div>

					<!-- Primary decision: audience -->
					<section>
						<h3 class="mb-1.5 text-sm font-semibold text-foreground">Who can see it</h3>
						<Select.Root type="single" bind:value={visibility}>
							<Select.Trigger class="h-10 w-full text-sm" aria-label="Share visibility">
								<span class="flex items-center gap-2">
									<VisibilityIcon class="size-4 text-muted-foreground" />
									{visibilityLabel}
								</span>
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

					<!-- Secondary options, unified into one consistent list so every
					     control reads the same way (icon + label left, control right). -->
					<section>
						<h3 class="mb-1.5 text-sm font-semibold text-foreground">Options</h3>
						<div class="divide-y divide-border-low/50 overflow-hidden rounded-lg border border-border-low/60 bg-background/45">
							<!-- Comments -->
							<div class="flex items-center justify-between gap-3 px-3 py-3">
								<div class="flex min-w-0 items-start gap-2.5">
									<MessageSquare class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
									<div class="min-w-0">
										<span class="block text-sm font-medium text-foreground">Viewer comments</span>
										<span class="block text-xs text-muted-foreground">Reactions stay on either way.</span>
									</div>
								</div>
								{@render toggle(commentsEnabled, () => (commentsEnabled = !commentsEnabled), "Allow viewer comments")}
							</div>

							<!-- Password (Pro) -->
							<div class="px-3 py-3">
								<div class="flex items-center justify-between gap-3">
									<div class="flex min-w-0 items-center gap-2.5">
										<KeyRound class="size-4 shrink-0 text-muted-foreground" />
										<span class="text-sm font-medium text-foreground">Password</span>
										{#if !isPro}<Badge variant="outline">Pro</Badge>{/if}
									</div>
									{#if isPro}
										{@render toggle(passwordEnabled, () => (passwordEnabled = !passwordEnabled), "Require a password")}
									{/if}
								</div>
								{#if isPro && passwordEnabled}
									<Input
										bind:value={password}
										type="password"
										placeholder="Set a password"
										class="mt-2.5 h-9"
									/>
								{:else if !isPro}
									<p class="mt-1 text-xs text-muted-foreground">Protect links with a password on Pro.</p>
								{/if}
							</div>

							<!-- Expiry (Pro) -->
							<div class="flex items-center justify-between gap-3 px-3 py-3">
								<div class="flex min-w-0 items-center gap-2.5">
									<CalendarClock class="size-4 shrink-0 text-muted-foreground" />
									<span class="text-sm font-medium text-foreground">Link expiry</span>
									{#if !isPro}<Badge variant="outline">Pro</Badge>{/if}
								</div>
								{#if isPro}
									<Select.Root type="single" bind:value={expiry}>
										<Select.Trigger class="h-9 w-36 text-sm" aria-label="Link expiry">
											{expiry === "never" ? "Never" : expiry === "7d" ? "7 days" : "30 days"}
										</Select.Trigger>
										<Select.Content>
											<Select.Item value="never">Never expires</Select.Item>
											<Select.Item value="7d">7 days</Select.Item>
											<Select.Item value="30d">30 days</Select.Item>
										</Select.Content>
									</Select.Root>
								{:else}
									<span class="shrink-0 text-sm text-muted-foreground">15 days</span>
								{/if}
							</div>
						</div>
						{#if !isPro}
							<p class="mt-1.5 text-[11px] text-muted-foreground">Free share links expire after 15 days.</p>
						{/if}
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

{#snippet toggle(on: boolean, onToggle: () => void, label: string)}
	<button
		type="button"
		role="switch"
		aria-checked={on}
		aria-label={label}
		onclick={onToggle}
		class={cn(
			"relative h-5 w-9 shrink-0 cursor-pointer rounded-full outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/50",
			on ? "bg-primary" : "bg-foreground/20",
		)}
	>
		<span
			class={cn(
				"absolute left-0.5 top-0.5 size-4 rounded-full bg-background shadow-sm transition-transform duration-200",
				on && "translate-x-4",
			)}
		></span>
	</button>
{/snippet}
