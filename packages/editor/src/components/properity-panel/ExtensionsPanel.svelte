<script lang="ts">
import { Blocks, ChevronRight, Download, Package, RefreshCw } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Input } from "@recast/ui/input";
import { SegmentedToggle } from "@recast/ui/segmented";
import { toast } from "@recast/ui/sonner";
import { Spinner } from "@recast/ui/spinner";
import { cn } from "@recast/ui/utils";
import { onMount } from "svelte";
import {
	installFromUrl,
	loadRegistryIndex,
	type RegistryIndexEntry,
	toggleExtension,
} from "../../lib/extensions";
import type { InstalledExtension } from "../../lib/wire-types";
import type { EditorStore } from "../../stores/editor-store.svelte";
import { extensionsStore } from "../../stores/extensions-store.svelte";
import ExtensionDetailsDialog from "./ExtensionDetailsDialog.svelte";
import { contribCount, countUpdates, updateAvailableFor } from "./extensions-panel.logic";
import PanelSection from "./PanelSection.svelte";

interface Props {
	/** Kept for API parity with the other panels (unused today). */
	store: EditorStore;
}
let { store: _store }: Props = $props();

let urlInput = $state("");
let installingUrl = $state(false);
let index = $state<RegistryIndexEntry[] | null>(null);
let loadingIndex = $state(false);

// Addressed by id, so the dialog resolves entry and installed state reactively without being re-opened.
let dialogOpen = $state(false);
let selectedId = $state<string | null>(null);

const entryById = $derived(new Map((index ?? []).map((e) => [e.id, e])));
const installedById = $derived(new Map(extensionsStore.installed.map((e) => [e.manifest.id, e])));
const selectedEntry = $derived(selectedId ? (entryById.get(selectedId) ?? null) : null);
const selectedInstalled = $derived(selectedId ? (installedById.get(selectedId) ?? null) : null);

/** Installed packs that have a newer version available in the registry. */
const updateCount = $derived(countUpdates(extensionsStore.installed, entryById));

function openDetails(id: string) {
	selectedId = id;
	dialogOpen = true;
}

/** Set when the registry could not be reached. `loadRegistryIndex` catches its
 *  own errors and returns null, so without this a failed fetch was
 *  indistinguishable from a genuinely empty registry. */
let indexFailed = $state(false);

async function loadGallery() {
	loadingIndex = true;
	indexFailed = false;
	try {
		const res = await loadRegistryIndex<{ extensions?: RegistryIndexEntry[] }>();
		if (res === null) {
			indexFailed = true;
			index = null;
			return;
		}
		index = res.extensions ?? [];
	} finally {
		loadingIndex = false;
	}
}

onMount(loadGallery);

async function onInstallUrl() {
	const url = urlInput.trim();
	if (!url || installingUrl) return;
	installingUrl = true;
	try {
		const ext = await installFromUrl(url);
		toast.success(`Installed ${ext.manifest.name}`);
		urlInput = "";
	} catch (err) {
		toast.error(`Install failed: ${err instanceof Error ? err.message : String(err)}`);
	} finally {
		installingUrl = false;
	}
}

/** Id of the pack currently updating inline, so its row button shows a
 *  spinner + "Updating…" rather than just going disabled. */
let updatingId = $state<string | null>(null);

/** Quick inline update straight from the installed row (no dialog detour). */
async function onQuickUpdate(ext: InstalledExtension) {
	const entry = entryById.get(ext.manifest.id);
	if (!entry || updatingId) return;
	updatingId = ext.manifest.id;
	try {
		const next = await installFromUrl(entry.manifestUrl);
		toast.success(`Updated ${next.manifest.name} to v${next.manifest.version}`);
	} catch (err) {
		toast.error(`Update failed: ${err instanceof Error ? err.message : String(err)}`);
	} finally {
		updatingId = null;
	}
}

async function onToggle(ext: InstalledExtension, enabled: boolean) {
	try {
		await toggleExtension(ext.manifest.id, enabled);
	} catch (err) {
		const verb = enabled ? "Enable" : "Disable";
		toast.error(`${verb} failed: ${err instanceof Error ? err.message : String(err)}`);
	}
}
</script>

<div class="flex flex-col gap-4 animate-in fade-in duration-200">
  <PanelSection
    title="Install from URL"
    hint="Asset packs add cursors, backgrounds, gradients and caption themes. Downloads are SHA-256 verified before install."
    flush
  >
    <div class="flex items-center gap-1.5">
      <Input
        type="url"
        bind:value={urlInput}
        placeholder="https://…/extension.json"
        spellcheck="false"
        aria-label="Extension manifest URL"
        class="h-7 min-w-0 flex-1 text-[11px]"
        onkeydown={(e) => {
          if (e.key === "Enter") onInstallUrl();
        }}
      />
      <Button
        size="sm"
        variant="secondary"
        class="h-7 gap-1 px-2 text-[11px]"
        disabled={!urlInput.trim() || installingUrl}
        onclick={onInstallUrl}
      >
        {#if installingUrl}
          <Spinner class="size-3" />
          Installing…
        {:else}
          <Download class="size-3" />
          Install
        {/if}
      </Button>
    </div>
    {#if extensionsStore.lastError}
      <p class="mt-1.5 text-[10px] text-destructive">{extensionsStore.lastError}</p>
    {/if}
  </PanelSection>

  <PanelSection title="Installed" flush>
    {#snippet action()}
      <span class="flex items-center gap-1.5">
        {#if updateCount > 0}
          <span
            class="rounded-full bg-primary/12 px-1.5 py-0.5 text-[9px] font-medium text-primary"
          >
            {updateCount} update{updateCount === 1 ? "" : "s"}
          </span>
        {/if}
        <span class="font-mono text-[10px] text-muted-foreground/70">
          {extensionsStore.installed.length}
        </span>
      </span>
    {/snippet}
    {#if extensionsStore.installed.length === 0}
      <p
        class="rounded-md border border-dashed border-border/60 px-2.5 py-3 text-center text-[10.5px] text-muted-foreground"
      >
        No extensions installed yet.
      </p>
    {:else}
      <div class="flex flex-col gap-1">
        {#each extensionsStore.installed as ext (ext.manifest.id)}
          {@const canUpdate = updateAvailableFor(ext, entryById)}
          <div
            class="flex items-center gap-2 rounded-md border border-border/60 bg-card/40 px-2 py-1.5"
          >
            <button
              type="button"
              class="flex min-w-0 flex-1 items-center gap-2 text-left"
              onclick={() => openDetails(ext.manifest.id)}
            >
              <Package class="size-3.5 shrink-0 text-muted-foreground" />
              <span class="min-w-0 flex-1">
                <span class="flex items-center gap-1.5">
                  <span class="truncate text-[11px] font-medium text-foreground">
                    {ext.manifest.name}
                  </span>
                  <span class="shrink-0 font-mono text-[9px] text-muted-foreground/70">
                    v{ext.manifest.version}
                  </span>
                </span>
                <span class="block text-[9.5px] text-muted-foreground/80">
                  {contribCount(ext)} item{contribCount(ext) === 1 ? "" : "s"}
                  {#if ext.manifest.author}· {ext.manifest.author}{/if}
                </span>
              </span>
            </button>
            {#if canUpdate}
              {@const isUpdating = updatingId === ext.manifest.id}
              <Button
                size="sm"
                class="h-6 gap-1 px-2 text-[10px]"
                disabled={extensionsStore.busy}
                aria-label={`Update ${ext.manifest.name}`}
                onclick={() => onQuickUpdate(ext)}
              >
                {#if isUpdating}
                  <Spinner class="size-3" />
                  Updating…
                {:else}
                  <Download class="size-3" />
                  Update
                {/if}
              </Button>
            {/if}
            <SegmentedToggle
              checked={ext.enabled}
              offLabel="Off"
              onLabel="On"
              size="xs"
              aria-label={`${ext.manifest.name} enabled`}
              onCheckedChange={(next) => onToggle(ext, next)}
            />
          </div>
        {/each}
      </div>
    {/if}
  </PanelSection>

  <PanelSection title="Browse" flush>
    {#snippet action()}
      <Button
        variant="ghost"
        size="xs"
        class="gap-1 text-[10.5px] text-muted-foreground"
        onclick={loadGallery}
        disabled={loadingIndex}
      >
        <RefreshCw class={cn("size-2.5", loadingIndex && "animate-spin")} />
        Refresh
      </Button>
    {/snippet}
    {#if loadingIndex && !index}
      <div class="flex items-center justify-center py-8">
        <Spinner class="size-5 text-muted-foreground" />
      </div>
    {:else if indexFailed}
      <div
        class="flex flex-col items-start gap-1.5 rounded-md border border-dashed border-border/60 px-2.5 py-3"
      >
        <p class="text-[10.5px] text-muted-foreground">
          Couldn't reach the extension registry. Check your connection, then try
          again.
        </p>
        <Button variant="outline" size="xs" class="gap-1" onclick={loadGallery}>
          <RefreshCw class="size-2.5" />
          Retry
        </Button>
      </div>
    {:else if !index || index.length === 0}
      <p
        class="rounded-md border border-dashed border-border/60 px-2.5 py-3 text-center text-[10.5px] text-muted-foreground"
      >
        No packs available right now.
      </p>
    {:else}
      <!-- PropertiesPanel owns the scroll container; this list just stacks. -->
      <div class="flex flex-col gap-1">
        {#each index as entry (entry.id)}
          {@const installedExt = installedById.get(entry.id)}
          {@const canUpdate = installedExt ? updateAvailableFor(installedExt, entryById) : false}
          <button
            type="button"
            class="flex w-full items-center gap-2 rounded-md border border-border/60 bg-card/40 px-2 py-1.5 text-left transition-colors hover:bg-card/70 focus:outline-none focus:ring-2 focus:ring-ring/40"
            onclick={() => openDetails(entry.id)}
          >
            {#if entry.iconUrl}
              <img
                src={entry.iconUrl}
                alt=""
                loading="lazy"
                class="size-7 shrink-0 rounded-md object-cover"
              />
            {:else}
              <div
                class="flex size-7 shrink-0 items-center justify-center rounded-md bg-muted/60"
              >
                <Blocks class="size-3.5 text-muted-foreground" />
              </div>
            {/if}
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[11px] font-medium text-foreground">
                {entry.name}
              </span>
              {#if entry.description}
                <span class="block truncate text-[9.5px] text-muted-foreground/80">
                  {entry.description}
                </span>
              {/if}
            </span>
            {#if canUpdate}
              <span class="shrink-0 text-[10px] font-medium text-primary">Update</span>
            {:else if installedExt}
              <span class="shrink-0 text-[10px] text-muted-foreground">Installed</span>
            {:else}
              <span
                class="inline-flex shrink-0 items-center gap-1 text-[10px] font-medium text-foreground"
              >
                <Download class="size-3" />
                Get
              </span>
            {/if}
            <ChevronRight class="size-3.5 shrink-0 text-muted-foreground/50" />
          </button>
        {/each}
      </div>
    {/if}
  </PanelSection>

  <ExtensionDetailsDialog
    bind:open={dialogOpen}
    entry={selectedEntry}
    installed={selectedInstalled}
  />
</div>
