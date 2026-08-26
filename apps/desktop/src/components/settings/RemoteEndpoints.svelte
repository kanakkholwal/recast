<script lang="ts">
import { Check, KeyRound, LoaderCircle, Pencil, Plus, Server, Trash2, X } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import { cn } from "@recast/ui/utils";
import { onMount } from "svelte";
import {
	deleteRemoteAsrEndpoint,
	listRemoteAsrEndpoints,
	type RemoteAsrEndpointInfo,
	setRemoteAsrEndpoint,
	setRemoteAsrKey,
} from "$lib/ipc";
import {
	type EndpointForm,
	emptyForm,
	formFromEndpoint,
	slugify,
	toEndpoint,
	validateForm,
} from "./remote-endpoints.logic";

/**
 * Manage OpenAI-compatible transcription endpoints. Config (name/URL/model) is
 * non-secret and returned by `list_remote_asr_endpoints`; the API key is
 * write-only via `set_remote_asr_key` (never read back), so the UI only shows
 * whether a key is stored.
 */
let endpoints = $state<RemoteAsrEndpointInfo[] | null>(null);
let form = $state<EndpointForm | null>(null);
/** Non-null while editing an existing endpoint (id is locked then). */
let editingId = $state<string | null>(null);
let keyInput = $state("");
/** Auto-derive the id from the name while adding, until the user edits it. */
let autoSlug = $state(true);
let busy = $state(false);

async function load() {
	try {
		endpoints = await listRemoteAsrEndpoints();
	} catch (e) {
		toast.error(`Couldn't load remote endpoints: ${e}`);
		endpoints = [];
	}
}

function openAdd() {
	form = emptyForm();
	editingId = null;
	keyInput = "";
	autoSlug = true;
}

function openEdit(ep: RemoteAsrEndpointInfo) {
	form = formFromEndpoint(ep);
	editingId = ep.id;
	keyInput = "";
	autoSlug = false;
}

function closeForm() {
	form = null;
	editingId = null;
	keyInput = "";
}

function onNameInput() {
	if (form && autoSlug && !editingId) form.id = slugify(form.displayName);
}

async function save() {
	if (!form || busy) return;
	const error = validateForm(form);
	if (error) {
		toast.error(error);
		return;
	}
	busy = true;
	try {
		const saved = await setRemoteAsrEndpoint(toEndpoint(form));
		// Only touch the key when the user typed one; blank on edit keeps the
		// stored key.
		if (keyInput.trim()) await setRemoteAsrKey(saved.id, keyInput.trim());
		toast.success(editingId ? "Endpoint updated" : "Endpoint added");
		closeForm();
		await load();
	} catch (e) {
		toast.error(String(e));
	} finally {
		busy = false;
	}
}

async function remove(ep: RemoteAsrEndpointInfo) {
	if (busy) return;
	busy = true;
	try {
		await deleteRemoteAsrEndpoint(ep.id);
		toast.success(`Removed ${ep.displayName}`);
		if (editingId === ep.id) closeForm();
		await load();
	} catch (e) {
		toast.error(String(e));
	} finally {
		busy = false;
	}
}

async function clearKey(ep: RemoteAsrEndpointInfo) {
	if (busy) return;
	busy = true;
	try {
		await setRemoteAsrKey(ep.id, "");
		toast.success("API key removed");
		await load();
	} catch (e) {
		toast.error(String(e));
	} finally {
		busy = false;
	}
}

onMount(load);
</script>

<div class="flex flex-col gap-3 px-4 py-3">
	{#if endpoints === null}
		<div class="flex items-center gap-2 text-[11.5px] text-muted-foreground">
			<LoaderCircle class="size-3.5 animate-spin" />
			<span>Loading endpoints…</span>
		</div>
	{:else}
		{#if endpoints.length === 0 && !form}
			<p class="text-[11.5px] text-muted-foreground">
				No endpoints yet. Add an OpenAI-compatible transcription endpoint to use
				it as a caption model.
			</p>
		{/if}

		{#each endpoints as ep (ep.id)}
			<div
				class="flex items-center gap-3 rounded-lg border border-border/40 bg-background/60 px-3 py-2"
			>
				<Server class="size-3.5 shrink-0 text-muted-foreground/70" />
				<div class="min-w-0 flex-1">
					<div class="truncate text-[12px] font-semibold text-foreground">
						{ep.displayName}
					</div>
					<div class="truncate font-mono text-[10.5px] text-muted-foreground">
						{ep.model} · {ep.baseUrl}
					</div>
				</div>
				<span
					class={cn(
						"inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[9.5px] font-semibold",
						ep.hasKey
							? "bg-success/12 text-success"
							: "bg-warning/12 text-warning",
					)}
				>
					<KeyRound class="size-2.5" />
					{ep.hasKey ? "Key saved" : "No key"}
				</span>
				{#if ep.hasKey}
					<Button
						variant="ghost"
						size="xs"
						class="h-7 gap-1 text-[11px] text-muted-foreground"
						disabled={busy}
						onclick={() => clearKey(ep)}
					>
						Remove key
					</Button>
				{/if}
				<Button
					variant="ghost"
					size="xs"
					class="size-7 shrink-0 p-0 text-muted-foreground"
					disabled={busy}
					aria-label={`Edit ${ep.displayName}`}
					onclick={() => openEdit(ep)}
				>
					<Pencil class="size-3.5" />
				</Button>
				<Button
					variant="ghost"
					size="xs"
					class="size-7 shrink-0 p-0 text-muted-foreground hover:text-destructive"
					disabled={busy}
					aria-label={`Remove ${ep.displayName}`}
					onclick={() => remove(ep)}
				>
					<Trash2 class="size-3.5" />
				</Button>
			</div>
		{/each}

		{#if form}
			<div class="flex flex-col gap-2.5 rounded-lg border border-border/60 bg-card/70 p-3">
				<div class="flex items-center justify-between">
					<span class="text-[12px] font-semibold text-foreground">
						{editingId ? "Edit endpoint" : "New endpoint"}
					</span>
					<Button
						variant="ghost"
						size="xs"
						class="size-6 p-0 text-muted-foreground"
						aria-label="Cancel"
						onclick={closeForm}
					>
						<X class="size-3.5" />
					</Button>
				</div>

				<label class="flex flex-col gap-1">
					<span class="text-[10.5px] font-medium text-muted-foreground">Name</span>
					<input
						bind:value={form.displayName}
						oninput={onNameInput}
						placeholder="LM Studio (local)"
						class="h-8 rounded-md border border-border/40 bg-background/60 px-2.5 text-[11.5px] text-foreground placeholder:text-muted-foreground/50 focus:border-border focus:outline-none"
					/>
				</label>

				<label class="flex flex-col gap-1">
					<span class="text-[10.5px] font-medium text-muted-foreground">
						Identifier
					</span>
					<input
						bind:value={form.id}
						oninput={() => (autoSlug = false)}
						disabled={!!editingId}
						placeholder="lmstudio-local"
						class="h-8 rounded-md border border-border/40 bg-background/60 px-2.5 font-mono text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:border-border focus:outline-none disabled:opacity-60"
					/>
				</label>

				<label class="flex flex-col gap-1">
					<span class="text-[10.5px] font-medium text-muted-foreground">
						Base URL
					</span>
					<input
						type="url"
						bind:value={form.baseUrl}
						placeholder="http://127.0.0.1:1234/v1"
						spellcheck="false"
						autocapitalize="off"
						autocomplete="off"
						class="h-8 rounded-md border border-border/40 bg-background/60 px-2.5 font-mono text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:border-border focus:outline-none"
					/>
				</label>

				<label class="flex flex-col gap-1">
					<span class="text-[10.5px] font-medium text-muted-foreground">Model</span>
					<input
						bind:value={form.model}
						placeholder="whisper-large-v3"
						spellcheck="false"
						autocapitalize="off"
						class="h-8 rounded-md border border-border/40 bg-background/60 px-2.5 font-mono text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:border-border focus:outline-none"
					/>
				</label>

				<label class="flex flex-col gap-1">
					<span class="text-[10.5px] font-medium text-muted-foreground">
						API key
						{#if editingId}
							<span class="font-normal text-muted-foreground/70">
								(leave blank to keep the saved key)
							</span>
						{/if}
					</span>
					<input
						type="password"
						bind:value={keyInput}
						placeholder="sk-…"
						spellcheck="false"
						autocapitalize="off"
						autocomplete="off"
						class="h-8 rounded-md border border-border/40 bg-background/60 px-2.5 font-mono text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:border-border focus:outline-none"
					/>
				</label>

				<div class="mt-0.5 flex items-center justify-end gap-2">
					<Button variant="ghost" size="sm" class="h-8" disabled={busy} onclick={closeForm}>
						Cancel
					</Button>
					<Button variant="secondary" size="sm" class="h-8 gap-1.5" disabled={busy} onclick={save}>
						{#if busy}
							<LoaderCircle class="size-3.5 animate-spin" />
						{:else}
							<Check class="size-3.5" />
						{/if}
						{editingId ? "Save changes" : "Add endpoint"}
					</Button>
				</div>
			</div>
		{:else}
			<Button variant="secondary" size="sm" class="h-8 w-full gap-1.5" onclick={openAdd}>
				<Plus class="size-3.5" />
				Add endpoint
			</Button>
		{/if}
	{/if}
</div>
