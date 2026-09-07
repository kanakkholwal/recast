<script lang="ts">
import {
	ArrowLeft,
	ChevronDown,
	LoaderCircle,
	Maximize2,
	Minimize2,
	PanelBottom,
	PanelRight,
	Redo2,
	RotateCcw,
	Save,
	Undo2,
	Upload,
	X,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { Kbd } from "@recast/ui/kbd";
import { Separator } from "@recast/ui/separator";
import * as Tooltip from "@recast/ui/tooltip";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import { chordLabel } from "../lib/host-hooks";
import type { EditorStore } from "../stores/editor-store.svelte";
import ConfirmDialog from "./dialog/ConfirmDialog.svelte";

interface Props {
	store: EditorStore;
	filename?: string;
	/** Host-supplied brand mark for the project menu trigger (the app owns its logo). */
	brand?: Snippet;
	onexport?: () => void;
	onsave?: () => void | Promise<void>;
	isSaving?: boolean;
	// Drives the Export button: export opens, close dismisses the picker, minimize sends it to the activity center, show reopens.
	/** `"none"` hides the control entirely, for a host with no export to run. */
	exportMode?: "export" | "close" | "minimize" | "show" | "none";
	/** Whether this editor's export is actively encoding (for the minimized
	 *  "Exporting…" label). */
	exportRunning?: boolean;
	showSidebar?: boolean;
	showTimeline?: boolean;
	onToggleSidebar?: () => void;
	onToggleTimeline?: () => void;
}

let {
	store,
	filename = "Recording",
	brand,
	onexport,
	onsave,
	isSaving = false,
	exportMode = "export",
	exportRunning = false,
	showSidebar = true,
	showTimeline = true,
	onToggleSidebar,
	onToggleTimeline,
}: Props = $props();

// The toggles are meaningless while the export surface owns the layout, so they disable rather than silently no-op.
const exportOpen = $derived(exportMode === "close" || exportMode === "minimize");

const toggleClass = (active: boolean) =>
	cn(
		"cursor-pointer flex size-6 items-center justify-center rounded transition-colors duration-150 active:scale-95",
		"disabled:pointer-events-none disabled:opacity-40",
		active
			? "bg-card text-foreground shadow-(--shadow-craft-inset)"
			: "text-muted-foreground hover:bg-card/60 hover:text-foreground",
	);
let showRevertConfirm = $state(false);

// The parent decides the action from exportMode; this just forwards the click.
function onExportClick() {
	onexport?.();
}
</script>

<div class="flex h-full w-full items-center gap-1.5 px-2 text-[11px]">
  <!-- Every Tooltip.Trigger below uses `child` to merge its props onto our own
       element. Without it the trigger renders a button AROUND the control:
       nested interactive elements, two tab stops each, and a disabled control
       whose tooltip still opens (the pointer falls through to the wrapper). -->
  <!-- Project menu: brand mark + name + chevron opens dashboard/preset actions,
       replacing the standalone back button and the centred Presets control. -->
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <button
          {...props}
          type="button"
          class="flex h-7 min-w-0 max-w-64 shrink-0 items-center gap-1.5 rounded-md px-1.5 text-[11px] font-semibold text-foreground transition-colors hover:bg-muted active:scale-[0.98]"
          aria-label="Project menu"
        >
          {#if brand}
            <span class="flex size-4 shrink-0 items-center justify-center">{@render brand()}</span>
          {/if}
          <span class="truncate tracking-tight" title={filename}>{filename}</span>
          {#if store.isDirty}
            <span class="size-1.5 shrink-0 rounded-full bg-primary" title="Unsaved changes"></span>
          {/if}
          <ChevronDown size={12} class="shrink-0 text-muted-foreground" />
        </button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content size="sm" align="start" class="w-56 text-[11px]">
      <DropdownMenu.Item>
        {#snippet child({ props })}
          <a {...props} href="/recasts">
            <ArrowLeft size={13} />
            Go to dashboard
          </a>
        {/snippet}
      </DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Root>

  <div class="ml-auto flex items-center gap-1.5">
    <div
      class="flex items-center gap-0.5 rounded-md bg-muted/40 p-0.5 ring-1 ring-inset ring-border/40"
    >
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <span {...props as Record<string, unknown>} class="inline-flex">
              <button
                type="button"
                onclick={() => store.undo()}
                disabled={exportOpen || !store.canUndo}
                aria-label="Undo"
                class={toggleClass(false)}
              >
                <Undo2 size={12} />
              </button>
            </span>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>
          <span class="inline-flex items-center gap-1.5">
            Undo <Kbd>{chordLabel("editor.undo")}</Kbd>
          </span>
        </Tooltip.Content>
      </Tooltip.Root>
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <span {...props as Record<string, unknown>} class="inline-flex">
              <button
                type="button"
                onclick={() => store.redo()}
                disabled={exportOpen || !store.canRedo}
                aria-label="Redo"
                class={toggleClass(false)}
              >
                <Redo2 size={12} />
              </button>
            </span>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>
          <span class="inline-flex items-center gap-1.5">
            Redo <Kbd>{chordLabel("editor.redo")}</Kbd>
          </span>
        </Tooltip.Content>
      </Tooltip.Root>
    </div>

    {#if store.canRevert}
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="ghost"
              size="xs"
              class="gap-1.5 text-[11px] text-muted-foreground hover:text-destructive"
              onclick={() => (showRevertConfirm = true)}
              disabled={isSaving}
              aria-label="Revert unsaved changes"
            >
              <RotateCcw size={12} />
              Revert
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>
          {isSaving
            ? "Saving. Wait for it to finish."
            : "Discard unsaved changes and restore the last saved state"}
        </Tooltip.Content>
      </Tooltip.Root>
    {/if}

    <Tooltip.Root>
      <Tooltip.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            variant={store.isDirty ? "secondary" : "ghost"}
            size="xs"
            class="gap-1.5 text-[11px]"
            onclick={() => onsave?.()}
            disabled={isSaving || (!store.isDirty && !isSaving)}
            aria-label={store.isDirty ? "Save project" : "Project saved"}
          >
            {#if isSaving}
              <LoaderCircle size={12} class="animate-spin" />
              Saving…
            {:else}
              <Save size={12} />
              {store.isDirty ? "Save" : "Saved"}
            {/if}
          </Button>
        {/snippet}
      </Tooltip.Trigger>
      <Tooltip.Content>
        {#if store.isDirty}
          <span class="inline-flex items-center gap-1.5">
            Save project <Kbd>{chordLabel("editor.save")}</Kbd>
          </span>
        {:else}
          No unsaved changes
        {/if}
      </Tooltip.Content>
    </Tooltip.Root>

    {#if exportMode === "close"}
      <Button
        onclick={onExportClick}
        variant="secondary"
        size="xs"
        aria-pressed="true"
        class="gap-1.5 text-[11px]"
      >
        <X size={12} />
        Close
      </Button>
    {:else if exportMode === "minimize"}
      <Button
        onclick={onExportClick}
        variant="secondary"
        size="xs"
        aria-pressed="true"
        class="gap-1.5 text-[11px]"
      >
        <Minimize2 size={12} />
        Minimize
      </Button>
    {:else if exportMode === "show"}
      <Button
        onclick={onExportClick}
        variant="secondary"
        size="xs"
        class="gap-1.5 text-[11px]"
      >
        {#if exportRunning}
          <LoaderCircle size={12} class="animate-spin" />
          Exporting…
        {:else}
          <Maximize2 size={12} />
          Show export
        {/if}
      </Button>
    {:else if exportMode === "export"}
      <Button onclick={onExportClick} size="xs" class="gap-1.5 text-[11px]">
        <Upload size={12} />
        Export
      </Button>
    {/if}
    <Separator orientation="vertical" class="mx-0.5 h-3.5" />

    <div
      class="flex items-center gap-0.5 rounded-md bg-muted/40 p-0.5 ring-1 ring-inset ring-border/40"
    >
      <Tooltip.Root>
        <Tooltip.Trigger>
          <!-- Span wrapper: a disabled <button> swallows hover, so without it the
               "Unavailable while…" branch below could never render. -->
          {#snippet child({ props })}
            <span {...props as Record<string, unknown>} class="inline-flex">
              <button
                type="button"
                onclick={() => onToggleTimeline?.()}
                disabled={exportOpen}
                aria-label="Toggle timeline"
                aria-pressed={!exportOpen && showTimeline}
                class={toggleClass(showTimeline)}
              >
                <PanelBottom size={12} />
              </button>
            </span>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>
          {#if exportOpen}
            Unavailable while the export panel is open
          {:else}
            <span class="inline-flex items-center gap-1.5">
              {showTimeline ? "Hide timeline" : "Show timeline"}
              <Kbd>{chordLabel("editor.toggleTimeline")}</Kbd>
            </span>
          {/if}
        </Tooltip.Content>
      </Tooltip.Root>

      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <span {...props as Record<string, unknown>} class="inline-flex">
              <button
                type="button"
                onclick={() => onToggleSidebar?.()}
                disabled={exportOpen}
                aria-label="Toggle properties panel"
                aria-pressed={!exportOpen && showSidebar}
                class={toggleClass(showSidebar)}
              >
                <PanelRight size={12} />
              </button>
            </span>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>
          {#if exportOpen}
            Unavailable while the export panel is open
          {:else}
            <span class="inline-flex items-center gap-1.5">
              {showSidebar ? "Hide properties" : "Show properties"}
              <Kbd>{chordLabel("editor.toggleSidebar")}</Kbd>
            </span>
          {/if}
        </Tooltip.Content>
      </Tooltip.Root>
    </div>


  </div>
</div>

<ConfirmDialog
  bind:open={showRevertConfirm}
  onOpenChange={(v) => (showRevertConfirm = v)}
  title="Revert unsaved changes?"
  description="Restores every setting to the last save. You can undo the revert with Ctrl+Z."
  confirmLabel="Revert"
  cancelLabel="Keep editing"
  variant="destructive"
  onConfirm={() => store.revertToSaved()}
/>
