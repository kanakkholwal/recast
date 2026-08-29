<script lang="ts">
import { AudioLines, ExternalLink, Plus, Repeat, Trash2, Volume2, VolumeX } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { SegmentedToggle } from "@recast/ui/segmented";
import { toast } from "@recast/ui/sonner";
import { clipDisplayName, collectCredits, pickAudioFile } from "../../lib/audio/music";
import { getEditorServices } from "../../lib/editor/services";
import type { EditorStore } from "../../stores/editor-store.svelte";
import PanelSection from "./PanelSection.svelte";
import SliderRow from "./SliderRow.svelte";

interface Props {
	store: EditorStore;
}
let { store }: Props = $props();

const shell = getEditorServices().shell;

const credits = $derived(collectCredits(store.musicClips));

async function addMusic() {
	try {
		const path = await pickAudioFile();
		if (path) store.addMusicClip({ kind: "local", path });
	} catch (error) {
		toast.error(`Could not add audio: ${error}`);
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

            <div class="mt-2 space-y-1.5">
              <SliderRow
                label="Volume"
                value={clip.gain}
                min={0}
                max={200}
                step={5}
                disabled={clip.muted}
                onstart={() => store.pushUndoState()}
                onchange={(v) => store.updateMusicClip(clip.id, { gain: v })}
                formatValue={(v) => `${v}%`}
              />
              <SliderRow
                label="Fade in"
                value={clip.fadeIn}
                min={0}
                max={5}
                step={0.1}
                onstart={() => store.pushUndoState()}
                onchange={(v) => store.updateMusicClip(clip.id, { fadeIn: v })}
                formatValue={(v) => `${v.toFixed(1)}s`}
              />
              <SliderRow
                label="Fade out"
                value={clip.fadeOut}
                min={0}
                max={5}
                step={0.1}
                onstart={() => store.pushUndoState()}
                onchange={(v) => store.updateMusicClip(clip.id, { fadeOut: v })}
                formatValue={(v) => `${v.toFixed(1)}s`}
              />
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
            {#if c.license && shell}
              <button
                type="button"
                class="mt-0.5 inline-flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground"
                onclick={() => c.license && void shell.openExternal(c.license)}
              >
                <ExternalLink size={9} /> View license
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    </PanelSection>
  {/if}

</div>
