<script lang="ts">
import type { IconComponent } from "@recast/icons";
import {
	Blend,
	Blocks,
	Captions,
	Download,
	FileBox,
	Image,
	MousePointer,
	Palette,
	ShieldCheck,
	Spline,
	Trash2,
	Waves,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as Dialog from "@recast/ui/dialog";
import { Kbd } from "@recast/ui/kbd";
import { SegmentedToggle } from "@recast/ui/segmented";
import { toast } from "@recast/ui/sonner";
import { Spinner } from "@recast/ui/spinner";
import { cn } from "@recast/ui/utils";
import {
	fetchManifestPreview,
	hasUpdate,
	installFromUrl,
	type RegistryIndexEntry,
	removeExtension,
	toggleExtension,
} from "../../lib/extensions";
import type {
	ExtensionContributions,
	ExtensionManifest,
	InstalledExtension,
} from "../../lib/wire-types";
import { extensionsStore } from "../../stores/extensions-store.svelte";
import { DIALOG_SURFACE } from "../dialog/dialog.styles";
import { buildContributionGroups } from "./extensions-panel.logic";

interface Props {
	open: boolean;
	/** Registry metadata (manifestUrl + version). Null for a local-only install. */
	entry: RegistryIndexEntry | null;
	/** The installed record, when this pack is installed. */
	installed: InstalledExtension | null;
}

let { open = $bindable(), entry, installed }: Props = $props();

// Installed packs already carry the manifest; for a registry entry we fetch it
// (the index only has summary metadata) so contents show before install.
let manifest = $state<ExtensionManifest | null>(null);
let loadingManifest = $state(false);
// Guards the load effect against re-fetching the same target on every re-run.
let loadKey = "";

$effect(() => {
	if (!open) {
		loadKey = "";
		return;
	}
	let key = "";
	if (installed) key = `i:${installed.manifest.id}:${installed.manifest.version}`;
	else if (entry) key = `e:${entry.id}:${entry.manifestUrl}`;
	if (key === loadKey) return;
	loadKey = key;

	if (installed) {
		manifest = installed.manifest;
		loadingManifest = false;
	} else if (entry) {
		manifest = null;
		loadingManifest = true;
		fetchManifestPreview(entry.manifestUrl)
			.then((m) => {
				manifest = m;
			})
			.finally(() => {
				loadingManifest = false;
			});
	}
});

const isInstalled = $derived(installed !== null);
const name = $derived(installed?.manifest.name ?? entry?.name ?? manifest?.name ?? "Extension");
const installedVersion = $derived(installed?.manifest.version);
const latestVersion = $derived(entry?.version ?? manifest?.version);
const author = $derived(installed?.manifest.author ?? entry?.author ?? manifest?.author ?? null);
// Only the registry entry carries a description; the manifest has none.
const description = $derived(entry?.description ?? null);
const updateAvailable = $derived(
	isInstalled && hasUpdate(installedVersion ?? "0.0.0", latestVersion),
);
const manifestUrl = $derived(entry?.manifestUrl ?? null);

// Icon-bearing definition table stays in the component; the pure map/filter
const contributionDefs: Array<{
	key: keyof ExtensionContributions;
	label: string;
	icon: IconComponent;
}> = [
	{ key: "cursors", label: "Cursors", icon: MousePointer },
	{ key: "backgrounds", label: "Backgrounds", icon: Image },
	{ key: "gradients", label: "Gradients", icon: Blend },
	{ key: "colors", label: "Colors", icon: Palette },
	{ key: "easings", label: "Easing presets", icon: Spline },
	{ key: "smoothings", label: "Smoothing presets", icon: Waves },
	{ key: "captionPresets", label: "Caption themes", icon: Captions },
];
const groups = $derived(buildContributionGroups(manifest, contributionDefs));

const assetCount = $derived(manifest?.assets?.length ?? 0);

// Previews resolve manifest-local asset ids to their remote source URLs
const contributes = $derived<ExtensionContributions>(manifest?.contributes ?? {});
const assetById = $derived(new Map((manifest?.assets ?? []).map((a) => [a.id, a])));
function assetUrl(id: string | null | undefined): string | null {
	return id ? (assetById.get(id)?.url ?? null) : null;
}
function bgThumbUrl(bg: { thumb?: string; asset: string }): string | null {
	const a = assetById.get(bg.thumb ?? bg.asset);
	return a?.thumbUrl ?? a?.url ?? null;
}

// Which action is in flight, so the matching button shows a spinner + verb.
// GOTCHA: `installed`/`entry` are reactive props that go null the moment the
// store updates, so handlers must capture any name/id for the toast BEFORE awaiting.
let pending = $state<null | "install" | "update" | "uninstall">(null);

async function onInstall() {
	if (!manifestUrl || pending) return;
	pending = "install";
	try {
		const ext = await installFromUrl(manifestUrl);
		toast.success(`Installed ${ext.manifest.name}`);
	} catch (err) {
		toast.error(`Install failed: ${err instanceof Error ? err.message : String(err)}`);
	} finally {
		pending = null;
	}
}

async function onUpdate() {
	if (!manifestUrl || pending) return;
	pending = "update";
	try {
		const ext = await installFromUrl(manifestUrl);
		toast.success(`Updated ${ext.manifest.name} to v${ext.manifest.version}`);
	} catch (err) {
		toast.error(`Update failed: ${err instanceof Error ? err.message : String(err)}`);
	} finally {
		pending = null;
	}
}

async function onUninstall() {
	if (!installed || pending) return;
	// Capture before await: `installed` goes null when the store drops the pack.
	const { id, name: packName } = installed.manifest;
	pending = "uninstall";
	try {
		await removeExtension(id);
		toast.success(`Removed ${packName}`);
		open = false;
	} catch (err) {
		toast.error(`Remove failed: ${err instanceof Error ? err.message : String(err)}`);
	} finally {
		pending = null;
	}
}

async function onToggle(next: boolean) {
	if (!installed) return;
	const { id } = installed.manifest;
	try {
		await toggleExtension(id, next);
	} catch (err) {
		toast.error(`Update failed: ${err instanceof Error ? err.message : String(err)}`);
	}
}
</script>

<Dialog.Root bind:open>
  <Dialog.Content
    showCloseButton={false}
    class={cn("top-[10%] w-[min(92vw,32rem)] max-w-none translate-y-0 gap-0 sm:max-w-none", DIALOG_SURFACE)}
  >
    <Dialog.Header class="space-y-0 border-b border-border px-4 py-2.5 text-left">
      <div class="flex items-center gap-2.5">
        {#if entry?.iconUrl}
          <img src={entry.iconUrl} alt="" loading="lazy" class="size-8 shrink-0 rounded-md object-cover" />
        {:else}
          <div
            class="flex size-8 shrink-0 items-center justify-center rounded-md border border-border/50 bg-muted/50"
          >
            <Blocks class="size-4 text-muted-foreground" />
          </div>
        {/if}
        <div class="min-w-0 flex-1">
          <Dialog.Title
            class="flex items-center gap-1.5 text-[13px] font-semibold tracking-tight text-foreground"
          >
            <span class="truncate">{name}</span>
            {#if installedVersion}
              <span class="shrink-0 font-mono text-[10px] font-normal text-muted-foreground/70">
                v{installedVersion}
              </span>
            {:else if latestVersion}
              <span class="shrink-0 font-mono text-[10px] font-normal text-muted-foreground/70">
                v{latestVersion}
              </span>
            {/if}
            {#if updateAvailable}
              <span
                class="shrink-0 rounded-full bg-primary/10 px-1.5 py-0.5 font-mono text-[9px] font-medium text-primary"
              >
                v{latestVersion}
              </span>
            {/if}
          </Dialog.Title>
          <Dialog.Description class="text-[11px] text-muted-foreground">
            {#if author}{author} · {/if}Asset pack
          </Dialog.Description>
        </div>
      </div>
    </Dialog.Header>

    <div class="max-h-[70vh] overflow-y-auto overflow-x-hidden">
      <div class="divide-y divide-border/30">
        <div class="px-4 py-3">
          {#if description}
            <p class="text-[11.5px] leading-relaxed text-pretty text-foreground/90">
              {description}
            </p>
          {/if}
          <div
            class="mt-2.5 flex items-center gap-1.5 rounded-lg border border-border/60 bg-card/40 px-2.5 py-1.5 text-[10.5px] text-muted-foreground shadow-(--shadow-craft-inset)"
          >
            <ShieldCheck class="size-3.5 shrink-0 text-muted-foreground" />
            <span>
              Runs no code. Every asset is downloaded over HTTPS and SHA-256
              verified before install.
            </span>
          </div>
        </div>

        {#if loadingManifest}
          <div class="flex items-center justify-center py-10">
            <Spinner class="size-5 text-muted-foreground" />
          </div>
        {:else if manifest}
          {#if groups.length > 0}
            {@const c = contributes}
            <div class="space-y-3.5 px-4 py-3">
              <h4
                class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
              >
                Includes
              </h4>

              {#snippet groupHead(Icon: IconComponent, label: string, count: number)}
                <div class="mb-1.5 flex items-center gap-1.5">
                  <Icon class="size-3.5 text-muted-foreground" />
                  <span class="text-[11px] font-medium text-foreground">{label}</span>
                  <span class="font-mono text-[9px] text-muted-foreground/70">{count}</span>
                </div>
              {/snippet}

              {#if c.cursors?.length}
                <section class="min-w-0">
                  {@render groupHead(MousePointer, "Cursors", c.cursors.length)}
                  <div class="flex flex-wrap gap-2">
                    {#each c.cursors as cur (cur.id)}
                      {@const url = assetUrl(cur.rest)}
                      <div class="flex w-14 flex-col items-center gap-1">
                        <div
                          class="grid size-12 place-items-center rounded-lg bg-muted/40 ring-1 ring-inset ring-border/40"
                        >
                          {#if url}
                            <img
                              src={url}
                              alt={cur.label}
                              loading="lazy"
                              class="size-8 object-contain"
                            />
                          {:else}
                            <MousePointer class="size-4 text-muted-foreground" />
                          {/if}
                        </div>
                        <span class="w-full truncate text-center text-[9px] text-muted-foreground">
                          {cur.label}
                        </span>
                      </div>
                    {/each}
                  </div>
                </section>
              {/if}

              {#if c.backgrounds?.length}
                <section class="min-w-0">
                  {@render groupHead(Image, "Backgrounds", c.backgrounds.length)}
                  <div class="flex flex-wrap gap-1.5">
                    {#each c.backgrounds as bg (bg.id)}
                      {@const url = bgThumbUrl(bg)}
                      <div class="w-24">
                        <div
                          class="aspect-video overflow-hidden rounded-md bg-muted/40 ring-1 ring-inset ring-border/40"
                        >
                          {#if url}
                            <img
                              src={url}
                              alt={bg.label}
                              loading="lazy"
                              class="size-full object-cover"
                            />
                          {/if}
                        </div>
                        <span class="mt-0.5 block truncate text-[9px] text-muted-foreground">
                          {bg.label}
                        </span>
                      </div>
                    {/each}
                  </div>
                </section>
              {/if}

              {#if c.gradients?.length}
                <section class="min-w-0">
                  {@render groupHead(Blend, "Gradients", c.gradients.length)}
                  <div class="flex flex-wrap gap-1.5">
                    {#each c.gradients as g (g.id)}
                      <div class="w-16">
                        <div
                          class="h-8 rounded-md ring-1 ring-inset ring-border/40"
                          style="background: {g.value}"
                        ></div>
                        <span class="mt-0.5 block truncate text-[9px] text-muted-foreground">
                          {g.label}
                        </span>
                      </div>
                    {/each}
                  </div>
                </section>
              {/if}

              {#if c.colors?.length}
                <section class="min-w-0">
                  {@render groupHead(Palette, "Colors", c.colors.length)}
                  <div class="flex flex-wrap items-center gap-1.5">
                    {#each c.colors as col (col.id)}
                      <span
                        class="size-6 rounded-md ring-1 ring-inset ring-border/40"
                        style="background: {col.value}"
                        title={col.label}
                      ></span>
                    {/each}
                  </div>
                </section>
              {/if}

              {#if c.easings?.length}
                <section class="min-w-0">
                  {@render groupHead(Spline, "Easing presets", c.easings.length)}
                  <div class="flex flex-col gap-1.5">
                    {#each c.easings as e (e.id)}
                      <div class="flex items-center gap-2">
                        <span class="w-24 shrink-0 truncate text-[10px] text-muted-foreground">
                          {e.label}
                        </span>
                        <!-- A dot runs the track with this curve's timing, so the
                             ease is felt as motion. Decorative; hidden from AT. -->
                        <div
                          aria-hidden="true"
                          class="relative h-4 flex-1 overflow-hidden rounded-full bg-muted/50 ring-1 ring-inset ring-border/30"
                        >
                          <span
                            class="ext-ease-dot absolute top-1/2 size-2 -translate-y-1/2 rounded-full bg-foreground"
                            style="animation-timing-function: cubic-bezier({e.value.x1}, {e.value.y1}, {e.value.x2}, {e.value.y2})"
                          ></span>
                        </div>
                      </div>
                    {/each}
                  </div>
                </section>
              {/if}

              {#if c.smoothings?.length}
                <section class="min-w-0">
                  {@render groupHead(Waves, "Smoothing presets", c.smoothings.length)}
                  <div class="flex flex-wrap gap-1">
                    {#each c.smoothings as s (s.id)}
                      <span
                        class="rounded-md border border-border/50 bg-muted/30 px-1.5 py-0.5 text-[10px] text-foreground/80"
                      >
                        {s.label}
                      </span>
                    {/each}
                  </div>
                </section>
              {/if}

              {#if c.captionPresets?.length}
                <section class="min-w-0">
                  {@render groupHead(Captions, "Caption themes", c.captionPresets.length)}
                  <div class="flex flex-wrap gap-1.5">
                    {#each c.captionPresets as cap (cap.id)}
                      <div
                        class="grid h-8 w-24 place-items-center overflow-hidden rounded-md bg-neutral-950 px-1"
                      >
                        <span
                          class="truncate text-[10px] font-semibold"
                          style="font-family: {cap.fontFamily}; color: {cap.color}; text-transform: {cap.uppercase
                            ? 'uppercase'
                            : 'none'}"
                        >
                          {cap.label}
                        </span>
                      </div>
                    {/each}
                  </div>
                </section>
              {/if}
            </div>
          {/if}

          {#if assetCount > 0}
            <div class="px-4 py-3">
              <div class="mb-1.5 flex items-center gap-1.5">
                <FileBox class="size-3.5 text-muted-foreground" />
                <span class="text-[11px] font-medium text-foreground">Assets</span>
                <span class="font-mono text-[9px] text-muted-foreground/70">{assetCount}</span>
              </div>
              <div class="flex flex-wrap gap-x-2 gap-y-0.5">
                {#each manifest.assets as a (a.id)}
                  <span class="font-mono text-[10px] text-muted-foreground/80">{a.filename}</span>
                {/each}
              </div>
            </div>
          {/if}

          {#if isInstalled}
            <div class="flex items-center justify-between gap-2 px-4 py-3">
              <span class="text-[11px] font-medium text-foreground">Enabled</span>
              <SegmentedToggle
                checked={installed?.enabled ?? false}
                offLabel="Off"
                onLabel="On"
                size="xs"
                aria-label={`${name} enabled`}
                onCheckedChange={onToggle}
              />
            </div>
          {/if}
        {:else}
          <div class="px-4 py-8 text-center text-[11px] text-muted-foreground">
            Couldn't load this extension's details.
          </div>
        {/if}
      </div>
    </div>

    <footer
      class="flex h-10 items-center justify-between gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
    >
      <div class="flex items-center gap-3">
        {#if isInstalled}
          <Button
            variant="destructive_soft"
            size="xs"
            disabled={extensionsStore.busy}
            onclick={onUninstall}
          >
            {#if pending === "uninstall"}
              <Spinner class="size-3" />
              Removing…
            {:else}
              <Trash2 class="size-3" />
              Uninstall
            {/if}
          </Button>
        {:else}
          <span class="flex items-center gap-1">
            <Kbd>Esc</Kbd>
            <span>Cancel</span>
          </span>
        {/if}
      </div>
      <div class="flex items-center gap-1.5">
        {#if isInstalled}
          {#if updateAvailable}
            <Button
              variant="default"
              size="xs"
              disabled={extensionsStore.busy}
              onclick={onUpdate}
            >
              {#if pending === "update"}
                <Spinner class="size-3" />
                Updating…
              {:else}
                <Download class="size-3" />
                Update to v{latestVersion}
              {/if}
            </Button>
          {/if}
          <Dialog.Close>
            {#snippet child({ props })}
              <Button variant="ghost" size="xs" {...props}>Done</Button>
            {/snippet}
          </Dialog.Close>
        {:else}
          <Dialog.Close>
            {#snippet child({ props })}
              <Button variant="ghost" size="xs" {...props}>Cancel</Button>
            {/snippet}
          </Dialog.Close>
          <Button
            variant="default"
            size="xs"
            disabled={!manifestUrl || extensionsStore.busy}
            onclick={onInstall}
          >
            {#if pending === "install"}
              <Spinner class="size-3" />
              Installing…
            {:else}
              <Download class="size-3" />
              Install
            {/if}
          </Button>
        {/if}
      </div>
    </footer>
  </Dialog.Content>
</Dialog.Root>

<style>
  .ext-ease-dot {
    animation: ext-ease-run 1.7s infinite;
  }
  @keyframes ext-ease-run {
    0%,
    12% {
      left: 0.25rem;
    }
    88%,
    100% {
      left: calc(100% - 0.75rem);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ext-ease-dot {
      animation: none;
      left: calc(100% - 0.75rem);
    }
  }
</style>
