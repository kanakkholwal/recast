<script lang="ts">
import { Check, LoaderCircle, RotateCcw, Server } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import { cn } from "@recast/ui/utils";
import { emit } from "@tauri-apps/api/event";
import { onMount } from "svelte";
import { type CloudApiConfig, getCloudApiConfig, setCloudApiUrl } from "$lib/ipc";

/**
 * Self-hosting server endpoint override. The Rust resolver
 * (`auth::cloud_api_url`) falls back to the default on an empty/malformed
 * value, so a bad entry can't brick sign-in; we validate on save too for an
 * explicit error. Changing the endpoint invalidates the session, so we emit
 * `cloud:endpoint-changed` to reset the sign-in card to signed-out.
 */
let config = $state<CloudApiConfig | null>(null);
let input = $state("");
let saving = $state(false);

// Dirty only when the trimmed input differs from what's persisted. An empty
// input with no override saved is not dirty (nothing to clear).
const dirty = $derived(config !== null && input.trim() !== (config.overrideUrl ?? ""));

async function load() {
	try {
		const c = await getCloudApiConfig();
		config = c;
		input = c.overrideUrl ?? "";
	} catch (e) {
		toast.error(`Couldn't load server endpoint: ${e}`);
	}
}

async function save() {
	if (saving) return;
	saving = true;
	const prevEffective = config?.effective;
	try {
		// Empty string clears the override → back to the default endpoint.
		const trimmed = input.trim();
		const next = await setCloudApiUrl(trimmed.length > 0 ? trimmed : null);
		config = next;
		input = next.overrideUrl ?? "";
		toast.success(
			next.isCustom
				? `Server endpoint set to ${next.effective}`
				: "Using the default Recast Cloud endpoint",
		);
		// Only nudge the sign-in card if the endpoint actually moved.
		if (next.effective !== prevEffective) {
			await emit("cloud:endpoint-changed");
		}
	} catch (e) {
		// `set_cloud_api_url` rejects invalid URLs with a friendly message.
		toast.error(String(e));
	} finally {
		saving = false;
	}
}

async function reset() {
	input = "";
	await save();
}

onMount(load);
</script>

<div class="flex flex-col gap-3 px-4 py-3">
	{#if config === null}
		<div class="flex items-center gap-2 text-[11.5px] text-muted-foreground">
			<LoaderCircle class="size-3.5 animate-spin" />
			<span>Loading endpoint…</span>
		</div>
	{:else}
		<div class="flex flex-col gap-1">
			<span class="text-[12px] font-semibold text-foreground">
				Server endpoint
			</span>
			<span class="text-[11px] text-muted-foreground">
				Self-hosting Recast Cloud? Point the app at your server. Changing
				this signs you out of the current server.
			</span>
		</div>

		<div class="flex items-center gap-2">
			<div
				class="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg border border-border/40 bg-background/60 px-3 focus-within:border-border"
			>
				<Server class="size-3.5 shrink-0 text-muted-foreground/70" />
				<input
					type="url"
					bind:value={input}
					placeholder={config.defaultUrl}
					spellcheck="false"
					autocapitalize="off"
					autocomplete="off"
					onkeydown={(e) => {
						if (e.key === "Enter" && dirty) save();
					}}
					class="min-w-0 flex-1 bg-transparent font-mono text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:outline-none"
				/>
			</div>
			<Button
				variant="secondary"
				size="sm"
				class="h-9 shrink-0 gap-1.5"
				disabled={!dirty || saving}
				onclick={save}
			>
				{#if saving}
					<LoaderCircle class="size-3.5 animate-spin" />
				{:else}
					<Check class="size-3.5" />
				{/if}
				Save
			</Button>
		</div>

		<div class="flex items-center justify-between gap-2">
			<span
				class={cn(
					"inline-flex items-center gap-1.5 text-[10.5px] font-medium",
					config.isCustom ? "text-warning" : "text-muted-foreground/70",
				)}
			>
				<span
					class={cn(
						"size-1.5 rounded-full",
						config.isCustom ? "bg-warning" : "bg-success",
					)}
				></span>
				{config.isCustom ? "Custom endpoint" : "Default endpoint"} ·
				<span class="font-mono">{config.effective}</span>
			</span>
			{#if config.isCustom}
				<Button
					variant="ghost"
					size="xs"
					class="h-6 gap-1.5 text-[11px]"
					disabled={saving}
					onclick={reset}
				>
					<RotateCcw class="size-3" />
					Reset to default
				</Button>
			{/if}
		</div>
	{/if}
</div>
