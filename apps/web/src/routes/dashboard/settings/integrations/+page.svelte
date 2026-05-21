<script lang="ts">
	import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
	import { settingsStore } from "$lib/dashboard/store.svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import { Check, Cloud, Database, Eye, EyeOff, Plug } from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly, slide } from "svelte/transition";

	const cloud = settingsStore.value.cloudinary;

	let showSecret = $state(false);

	const cloudinaryReady = $derived(
		cloud.cloudName.trim() !== "" &&
			cloud.apiKey.trim() !== "" &&
			cloud.apiSecret.trim() !== "",
	);

	const inputClass =
		"rounded-lg border border-border-low/70 bg-background/80 px-3 py-2 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/60 focus:border-primary/60";

	function connect(e: SubmitEvent) {
		e.preventDefault();
		if (!cloudinaryReady) return;
		cloud.connected = true;
		settingsStore.save();
		toast.success("Cloudinary connected — credentials saved on this device.");
	}

	function disconnect() {
		cloud.connected = false;
		settingsStore.save();
		toast.info("Cloudinary disconnected.");
	}
</script>

<div class="flex flex-col gap-4">
	<!-- Cloudinary -->
	<div in:fly={{ y: 14, duration: 420, easing: cubicOut }}>
		<SettingsSection
			icon={Cloud}
			title="Cloudinary"
			description="Upload and serve your recasts from your own Cloudinary account."
			accent
		>
			{#snippet badge()}
				{#if cloud.connected}
					<span
						class="flex items-center gap-1.5 rounded-full bg-primary/12 px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-primary"
						in:fly={{ y: 4, duration: 240, easing: cubicOut }}
					>
						<Check class="size-3" />
						Connected
					</span>
				{:else}
					<span class="rounded-full bg-foreground/8 px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
						Not connected
					</span>
				{/if}
			{/snippet}

			{#if cloud.connected}
				<div
					class="flex items-center justify-between rounded-lg border border-primary/20 bg-primary/6 px-4 py-3"
					transition:slide={{ duration: 260, easing: cubicOut }}
				>
					<div class="text-sm">
						<span class="font-semibold text-foreground">{cloud.cloudName}</span>
						<span class="text-muted-foreground"> · default upload target</span>
					</div>
					<Button variant="ghost" size="sm" onclick={disconnect}>Disconnect</Button>
				</div>
			{:else}
				<form
					class="grid gap-4 sm:grid-cols-2"
					onsubmit={connect}
					transition:slide={{ duration: 260, easing: cubicOut }}
				>
					<label class="flex flex-col gap-1.5">
						<span class="text-xs font-semibold text-foreground/85">Cloud name</span>
						<input type="text" required bind:value={cloud.cloudName} placeholder="my-studio" class={inputClass} />
					</label>
					<label class="flex flex-col gap-1.5">
						<span class="text-xs font-semibold text-foreground/85">Upload preset</span>
						<input type="text" bind:value={cloud.uploadPreset} placeholder="recast_unsigned" class={inputClass} />
					</label>
					<label class="flex flex-col gap-1.5">
						<span class="text-xs font-semibold text-foreground/85">API key</span>
						<input type="text" required bind:value={cloud.apiKey} placeholder="123456789012345" class="{inputClass} font-mono" />
					</label>
					<label class="flex flex-col gap-1.5">
						<span class="text-xs font-semibold text-foreground/85">API secret</span>
						<div class="flex items-center rounded-lg border border-border-low/70 bg-background/80 pr-1 transition-colors focus-within:border-primary/60">
							<input
								type={showSecret ? "text" : "password"}
								required
								bind:value={cloud.apiSecret}
								placeholder="••••••••••••••••"
								class="w-full bg-transparent px-3 py-2 font-mono text-sm text-foreground outline-none placeholder:text-muted-foreground/60"
							/>
							<button
								type="button"
								onclick={() => (showSecret = !showSecret)}
								aria-label={showSecret ? "Hide secret" : "Show secret"}
								class="grid size-7 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
							>
								{#if showSecret}
									<EyeOff class="size-3.5" />
								{:else}
									<Eye class="size-3.5" />
								{/if}
							</button>
						</div>
					</label>

					<div class="flex items-center gap-3 sm:col-span-2">
						<Button type="submit" size="sm" class="gap-2" disabled={!cloudinaryReady}>
							<Plug class="size-3.5" />
							Connect Cloudinary
						</Button>
						<span class="text-xs text-muted-foreground">
							Credentials stay in this browser — nothing is sent anywhere.
						</span>
					</div>
				</form>
			{/if}
		</SettingsSection>
	</div>

	<!-- Amazon S3 — deprioritised -->
	<div in:fly={{ y: 14, duration: 420, delay: 80, easing: cubicOut }}>
		<SettingsSection
			icon={Database}
			title="Amazon S3"
			description="Bring your own S3 bucket for full control over hosting."
			tone="muted"
		>
			{#snippet badge()}
				<span class="rounded-full bg-foreground/8 px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
					Coming soon
				</span>
			{/snippet}

			<div class="grid gap-4 opacity-55 sm:grid-cols-2">
				{#each [{ label: "Bucket name", ph: "recast-recasts" }, { label: "Region", ph: "us-east-1" }, { label: "Access key ID", ph: "AKIA…" }, { label: "Secret access key", ph: "••••••••" }] as field (field.label)}
					<label class="flex flex-col gap-1.5">
						<span class="text-xs font-semibold text-foreground/85">{field.label}</span>
						<input
							type="text"
							disabled
							placeholder={field.ph}
							class="cursor-not-allowed rounded-lg border border-border-low/70 bg-background/50 px-3 py-2 text-sm text-foreground outline-none placeholder:text-muted-foreground/50"
						/>
					</label>
				{/each}
			</div>
			<p class="mt-4 text-xs text-muted-foreground">
				S3 support lands after Cloudinary. Want it sooner?
				<button
					type="button"
					class="font-semibold text-primary hover:underline"
					onclick={() => toast.info("Noted — S3 is on the roadmap.")}
				>
					Let us know.
				</button>
			</p>
		</SettingsSection>
	</div>
</div>
