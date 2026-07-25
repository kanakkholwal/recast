<script lang="ts">
  import type { EditorStore } from "$lib/stores/editor-store.svelte";
  import {
    clipDisplayName,
    collectCredits,
    pickAudioFile,
    type MusicSearchResult,
  } from "$lib/audio/music";
  import { createJamendoProvider } from "$lib/audio/providers/jamendo";
  import { clock } from "$lib/format/time";
  import {
    AudioLines,
    Download,
    ExternalLink,
    Loader2,
    Pause,
    Play,
    Plus,
    Repeat,
    Search,
    Trash2,
    Volume2,
    VolumeX,
  } from "@recast/icons";
  import { onDestroy } from "svelte";
  import { Button } from "@recast/ui/button";
  import { SegmentedToggle } from "@recast/ui/segmented";
  import { SliderControl } from "@recast/ui/slider-control";
  import { toast } from "@recast/ui/sonner";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import PanelSection from "./PanelSection.svelte";

  interface Props {
    store: EditorStore;
  }
  let { store }: Props = $props();

  const credits = $derived(collectCredits(store.musicClips));

  async function addMusic() {
    try {
      const path = await pickAudioFile();
      if (path) store.addMusicClip({ kind: "local", path });
    } catch (error) {
      toast.error(`Could not add audio: ${error}`);
    }
  }

  // Jamendo browse. The client ID is a free, non-secret catalog key; persist it
  // locally so it's entered once. All Jamendo tracks are Creative Commons — the
  // downloaded clip carries its attribution for crediting.
  const CLIENT_ID_KEY = "recast:jamendoClientId";
  function loadClientId(): string {
    return typeof localStorage !== "undefined" ? (localStorage.getItem(CLIENT_ID_KEY) ?? "") : "";
  }
  // `clientId` = the committed key; `clientIdDraft` = the input. Only "Set"
  // commits, so typing doesn't half-save a partial key.
  let clientId = $state(loadClientId());
  let clientIdDraft = $state(loadClientId());
  function setClientId() {
    const v = clientIdDraft.trim();
    if (!v) return;
    clientId = v;
    if (typeof localStorage !== "undefined") localStorage.setItem(CLIENT_ID_KEY, v);
  }
  function clearClientId() {
    stopPreview();
    clientId = "";
    clientIdDraft = "";
    results = [];
    if (typeof localStorage !== "undefined") localStorage.removeItem(CLIENT_ID_KEY);
  }

  let query = $state("");
  let searching = $state(false);
  let results = $state<MusicSearchResult[]>([]);
  let addingId = $state<string | null>(null);

  // Audition a result by streaming it (no download). One at a time.
  let previewEl = $state<HTMLAudioElement | null>(null);
  let playingId = $state<string | null>(null);
  function togglePreview(r: MusicSearchResult) {
    const url = r.previewUrl ?? r.downloadUrl;
    if (!previewEl || !url) return;
    if (playingId === r.trackId) {
      previewEl.pause();
      playingId = null;
      return;
    }
    previewEl.src = url;
    playingId = r.trackId;
    void previewEl.play().catch(() => {
      playingId = null;
      toast.error("Couldn't play preview.");
    });
  }
  function stopPreview() {
    if (previewEl) previewEl.pause();
    playingId = null;
  }
  onDestroy(stopPreview);

  async function runSearch() {
    const q = query.trim();
    if (!q) return;
    if (!clientId.trim()) {
      toast.error("Add your Jamendo client ID first.");
      return;
    }
    searching = true;
    try {
      results = await createJamendoProvider(clientId.trim()).search(q);
      if (results.length === 0) toast.info("No tracks found.");
    } catch (error) {
      toast.error(`${(error as Error)?.message ?? error}`);
    } finally {
      searching = false;
    }
  }

  async function addResult(result: MusicSearchResult) {
    addingId = result.trackId;
    try {
      const source = await createJamendoProvider(clientId.trim()).resolve(result);
      store.addMusicClip(source);
      toast.success(`Added "${result.title}".`);
    } catch (error) {
      toast.error(`Couldn't add track: ${(error as Error)?.message ?? error}`);
    } finally {
      addingId = null;
    }
  }
</script>

<div class="flex flex-col gap-1">
  <PanelSection
    title="Music"
    hint="Add a background track or voiceover over the whole edit. Mixed in on export."
    flush
  >
    {#snippet action()}
      <Button variant="ghost" size="icon-sm" onclick={addMusic} aria-label="Add music">
        <Plus size={14} />
      </Button>
    {/snippet}

    {#if store.musicOnlyClips.length === 0}
      <button
        type="button"
        onclick={addMusic}
        class="flex w-full flex-col items-center gap-1.5 rounded-md border border-dashed border-border/70 bg-card/40 px-3 py-5 text-muted-foreground transition-colors hover:border-border hover:text-foreground"
      >
        <AudioLines size={18} />
        <span class="text-[11px] font-medium">Add music or voiceover</span>
        <span class="text-[10px] text-muted-foreground/70">mp3, wav, m4a, aac, ogg, flac</span>
      </button>
    {:else}
      <div class="flex flex-col gap-2.5">
        {#each store.musicOnlyClips as clip (clip.id)}
          <div class="rounded-lg border border-border/60 bg-card/40 p-2.5">
            <div class="flex items-center gap-2">
              <AudioLines size={13} class="shrink-0 text-muted-foreground" />
              <span class="min-w-0 flex-1 truncate text-[11px] font-medium" title={clipDisplayName(clip)}>
                {clipDisplayName(clip)}
              </span>
              <button
                type="button"
                aria-label={clip.muted ? "Unmute" : "Mute"}
                title={clip.muted ? "Unmute" : "Mute"}
                class="rounded p-1 text-muted-foreground hover:text-foreground"
                onclick={() => store.updateMusicClip(clip.id, { muted: !clip.muted })}
              >
                {#if clip.muted}<VolumeX size={13} />{:else}<Volume2 size={13} />{/if}
              </button>
              <button
                type="button"
                aria-label="Remove"
                title="Remove"
                class="rounded p-1 text-muted-foreground hover:text-destructive"
                onclick={() => store.removeMusicClip(clip.id)}
              >
                <Trash2 size={13} />
              </button>
            </div>

            <div class="mt-2 space-y-2">
              <SliderControl
                label="Volume"
                value={clip.gain}
                min={0}
                max={200}
                step={5}
                unit="%"
                disabled={clip.muted}
                onstart={() => store.pushUndoState()}
                onchange={(v) => store.updateMusicClip(clip.id, { gain: v })}
                formatValue={(v) => `${v}%`}
              />
              <div class="grid grid-cols-2 gap-2">
                <SliderControl
                  label="Fade in"
                  value={clip.fadeIn}
                  min={0}
                  max={5}
                  step={0.1}
                  unit="s"
                  onstart={() => store.pushUndoState()}
                  onchange={(v) => store.updateMusicClip(clip.id, { fadeIn: v })}
                  formatValue={(v) => `${v.toFixed(1)}s`}
                />
                <SliderControl
                  label="Fade out"
                  value={clip.fadeOut}
                  min={0}
                  max={5}
                  step={0.1}
                  unit="s"
                  onstart={() => store.pushUndoState()}
                  onchange={(v) => store.updateMusicClip(clip.id, { fadeOut: v })}
                  formatValue={(v) => `${v.toFixed(1)}s`}
                />
              </div>
              <div class="flex items-center justify-between pt-0.5">
                <span class="inline-flex items-center gap-1.5 text-[11px] text-foreground">
                  <Repeat size={12} /> Loop to fill
                </span>
                <SegmentedToggle
                  checked={clip.loop}
                  size="xs"
                  aria-label="Loop to fill the video"
                  onCheckedChange={(next) => store.updateMusicClip(clip.id, { loop: next })}
                />
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </PanelSection>

  {#if credits.length > 0}
    <PanelSection
      title="Credits"
      hint="Attribution required by these tracks' licenses. Also written to the exported file's metadata."
      flush
    >
      <ul class="flex flex-col gap-1.5">
        {#each credits as c (c.id)}
          <li
            class="rounded-md border border-border/50 bg-card/40 px-2 py-1.5 text-[11px] text-foreground"
          >
            <div class="leading-snug">{c.attribution}</div>
            {#if c.license}
              <button
                type="button"
                class="mt-0.5 inline-flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground"
                onclick={() => c.license && void openUrl(c.license)}
              >
                <ExternalLink size={9} /> View license
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    </PanelSection>
  {/if}

  <PanelSection
    title="Browse Jamendo"
    hint="Free Creative Commons music. Credit is required — each added clip keeps its attribution."
    flush
    collapsible
    defaultOpen={false}
  >
    {#if !clientId.trim()}
      <form
        class="flex flex-col gap-1.5"
        onsubmit={(e) => {
          e.preventDefault();
          setClientId();
        }}
      >
        <label class="text-[11px] text-muted-foreground" for="jamendo-key">Jamendo client ID</label>
        <div class="flex items-center gap-1.5">
          <input
            id="jamendo-key"
            bind:value={clientIdDraft}
            placeholder="Paste your free client ID"
            class="min-w-0 flex-1 rounded-md border border-border/60 bg-background/60 px-2 py-1.5 text-[11px] outline-none focus:border-border"
          />
          <Button type="submit" size="sm" disabled={!clientIdDraft.trim()}>Set</Button>
        </div>
        <button
          type="button"
          class="inline-flex items-center gap-1 self-start text-[10px] text-primary hover:underline"
          onclick={() => void openUrl("https://devportal.jamendo.com/")}
        >
          <ExternalLink size={10} /> Get a free client ID
        </button>
      </form>
    {:else}
      <form
        class="flex items-center gap-1.5"
        onsubmit={(e) => {
          e.preventDefault();
          void runSearch();
        }}
      >
        <div class="relative flex-1">
          <Search size={12} class="absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            bind:value={query}
            placeholder="Search music…"
            class="w-full rounded-md border border-border/60 bg-background/60 py-1.5 pl-7 pr-2 text-[11px] outline-none focus:border-border"
          />
        </div>
        <Button type="submit" size="sm" disabled={searching}>
          {#if searching}<Loader2 size={13} class="animate-spin" />{:else}Search{/if}
        </Button>
      </form>

      {#if results.length > 0}
        <div class="mt-2 flex flex-col gap-1">
          {#each results as r (r.trackId)}
            <div class="flex items-center gap-2 rounded-md border border-border/50 bg-card/40 px-2 py-1.5">
              <button
                type="button"
                aria-label={playingId === r.trackId ? `Pause ${r.title}` : `Play ${r.title}`}
                title="Preview"
                class="rounded p-1 text-muted-foreground hover:text-foreground"
                onclick={() => togglePreview(r)}
              >
                {#if playingId === r.trackId}<Pause size={14} />{:else}<Play size={14} />{/if}
              </button>
              <div class="min-w-0 flex-1">
                <div class="truncate text-[11px] font-medium" title={r.title}>{r.title}</div>
                <div class="truncate text-[10px] text-muted-foreground">
                  {r.artist}{#if r.durationSec}
                    · {clock(r.durationSec)}{/if}
                </div>
              </div>
              <button
                type="button"
                aria-label={`Add ${r.title}`}
                title="Add to timeline"
                disabled={addingId === r.trackId}
                class="rounded p-1 text-muted-foreground hover:text-foreground disabled:opacity-50"
                onclick={() => void addResult(r)}
              >
                {#if addingId === r.trackId}<Loader2 size={14} class="animate-spin" />{:else}<Download
                    size={14}
                  />{/if}
              </button>
            </div>
          {/each}
        </div>
      {/if}
      <button
        type="button"
        class="mt-1.5 text-[10px] text-muted-foreground hover:text-foreground"
        onclick={clearClientId}
      >
        Change client ID
      </button>
    {/if}

    <!-- Hidden audition player: streams a result without downloading. -->
    <audio bind:this={previewEl} onended={() => (playingId = null)} class="hidden"></audio>
  </PanelSection>
</div>
