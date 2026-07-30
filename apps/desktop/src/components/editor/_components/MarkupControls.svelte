<script lang="ts">
import { insertImageAnnotation } from "$lib/annotations/image-import";
import {
	ANNOTATION_TOOLS,
	IMAGE_TOOL,
	toolForHotkey,
	type AnnotationToolId,
} from "$lib/annotations/tools";
import { isEditableTarget } from "$lib/dom/editable";
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { Eye, EyeOff } from "@recast/icons";
import * as Tooltip from "@recast/ui/tooltip";
import { cn } from "@recast/ui/utils";
import { BAR_BTN, BAR_GROUP } from "./player-bar.styles";

interface Props {
	store: EditorStore;
}

let { store }: Props = $props();

const ImageToolIcon = IMAGE_TOOL.icon;
const count = $derived(store.annotations.length);
const hidden = $derived(store.annotationsGloballyHidden);
const onTab = $derived(store.activePanel === "annotations");

function pickTool(id: AnnotationToolId) {
	if (id === "select") {
		store.annotationTool = null;
		return;
	}
	store.annotationTool = store.annotationTool === id ? null : id;
}

function isActive(id: AnnotationToolId): boolean {
	return id === "select" ? store.annotationTool === null : store.annotationTool === id;
}

function toggleHide() {
	store.annotationsGloballyHidden = !store.annotationsGloballyHidden;
}

// Tool hotkeys live with the tools they drive. Unlike the markup panel, this
// component stays mounted on every tab, so the tab check is explicit rather
// than a side effect of when it happens to be rendered.
function handleHotkey(event: KeyboardEvent) {
	if (!onTab) return;
	if (event.metaKey || event.ctrlKey || event.altKey) return;
	if (isEditableTarget(event.target)) return;
	const key = event.key.toLowerCase();
	if (key === IMAGE_TOOL.hotkey.toLowerCase()) {
		event.preventDefault();
		void insertImageAnnotation(store);
		return;
	}
	const tool = toolForHotkey(key);
	if (!tool) return;
	event.preventDefault();
	pickTool(tool.id);
}
</script>

<!-- `<svelte:window>` so HMR rebinds rather than leaking the listener. -->
<svelte:window onkeydown={handleHotkey} />

<!-- Drawing tools sit on the player bar, one row under the picture, rather than
     floating over it: markup is placed by eye and a pill over the frame covers
     the top-centre of the very content being annotated. -->
{#if onTab}
  <div class={cn(BAR_GROUP, "shrink-0")}>
    {#each ANNOTATION_TOOLS as t, i (t.id)}
      {@const Icon = t.icon}
      {@const active = isActive(t.id)}
      <!-- Select is the way out of every drawing mode, so it reads as its own
           group, as it does in Figma's toolbar. -->
      {#if i === 1}
        <span class="mx-0.5 h-4 w-px bg-border/60" aria-hidden="true"></span>
      {/if}
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <button
              {...props as Record<string, unknown>}
              type="button"
              onclick={() => pickTool(t.id)}
              aria-label={t.label}
              aria-pressed={active}
              class={cn(
                BAR_BTN,
                // A filled accent, not the bar's raised pill: an armed tool
                // changes what a click on the picture does, which is a stronger
                // claim than "this view option is on".
                active && "bg-primary text-primary-foreground hover:bg-primary hover:text-primary-foreground",
              )}
            >
              <Icon size={13} />
            </button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>{t.label} · {t.hotkey}</Tooltip.Content>
      </Tooltip.Root>
    {/each}

    <span class="mx-0.5 h-4 w-px bg-border/60" aria-hidden="true"></span>

    <!-- One-shot insert, never a mode, so it never takes the pressed state. -->
    <Tooltip.Root>
      <Tooltip.Trigger>
        {#snippet child({ props })}
          <button
            {...props as Record<string, unknown>}
            type="button"
            onclick={() => insertImageAnnotation(store)}
            aria-label={IMAGE_TOOL.label}
            class={BAR_BTN}
          >
            <ImageToolIcon size={13} />
          </button>
        {/snippet}
      </Tooltip.Trigger>
      <Tooltip.Content>{IMAGE_TOOL.label} · {IMAGE_TOOL.hotkey}</Tooltip.Content>
    </Tooltip.Root>

    {#if count > 0}
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <button
              {...props as Record<string, unknown>}
              type="button"
              onclick={toggleHide}
              aria-label="Markup visible"
              aria-pressed={!hidden}
              class={BAR_BTN}
            >
              {#if hidden}
                <EyeOff size={13} />
              {:else}
                <Eye size={13} />
              {/if}
            </button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>{hidden ? "Show markup" : "Hide markup"}</Tooltip.Content>
      </Tooltip.Root>
    {/if}
  </div>
{/if}

<!-- Hidden markup silently changes the exported file, so it is reported on every
     tab, in words. The warning tint only reinforces what the words already say.
     Terse on the Markup tab, where the eye toggle beside it already says
     "hidden" and the long form would crowd the centred transport. -->
{#if hidden && count > 0}
  <button
    type="button"
    onclick={toggleHide}
    aria-label="Markup is hidden and will not be exported. Show it."
    class="flex h-7 shrink-0 cursor-pointer items-center gap-1.5 rounded-lg bg-warning/15 px-2 text-[11px] font-medium text-foreground ring-1 ring-inset ring-border/40 transition-colors duration-150 hover:bg-warning/25"
  >
    {#if onTab}
      Not exported
    {:else}
      <EyeOff size={12} class="text-muted-foreground" />
      Markup hidden — not exported
    {/if}
  </button>
{/if}
