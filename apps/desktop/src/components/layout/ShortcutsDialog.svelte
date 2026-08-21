<script lang="ts">
import * as Dialog from "@recast/ui/dialog";
import { DIALOG_SURFACE } from "@recast/editor/components/dialog/dialog.styles";
import { cn } from "@recast/ui/utils";
import { Kbd } from "@recast/ui/kbd";
import { Keyboard } from "@recast/icons";
import {
	formatChordTokens,
	shortcutDefs,
	shortcutsByCategory,
	shortcutsDialog,
	type ShortcutDef,
} from "$lib/shortcuts/registry.svelte";

// The list is static (declared once in the registry); compute the grouping
// and the Mod+/ display tokens a single time.
const groups = shortcutsByCategory();
const openTokens = formatChordTokens("Mod+/");

function tokensFor(def: ShortcutDef): string[] {
	return def.display ?? formatChordTokens(def.keys);
}
</script>

<Dialog.Root bind:open={shortcutsDialog.open}>
  <Dialog.Content
    showCloseButton={false}
    class={cn("top-[6%] w-[min(92vw,52rem)] max-w-none translate-y-0 sm:max-w-none", DIALOG_SURFACE)}
  >
    <Dialog.Header class="border-b border-border px-4 py-2.5">
      <Dialog.Title
        class="flex items-center gap-2 text-[13px] font-semibold tracking-tight text-foreground"
      >
        <Keyboard class="size-4 text-muted-foreground" />
        Keyboard shortcuts
      </Dialog.Title>
      <Dialog.Description class="flex items-center gap-1.5 text-[11px] text-muted-foreground">
        Press
        {#each openTokens as t (t)}
          <Kbd>{t}</Kbd>
        {/each}
        anytime to open this. Editor, timeline and tool shortcuts apply only on
        their surface.
      </Dialog.Description>
    </Dialog.Header>

    <div class="max-h-[72vh] overflow-y-auto overflow-x-hidden px-5 py-4">
      <div class="grid grid-cols-1 gap-x-10 gap-y-6 sm:grid-cols-2">
        {#each groups as [category, defs] (category)}
          <section class="min-w-0">
            <h3
              class="mb-2 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
            >
              {category}
            </h3>
            <ul class="flex flex-col gap-0.5">
              {#each defs as def (def.id)}
                <li
                  class="flex items-start justify-between gap-3 rounded-md px-2 py-1.5 transition-colors hover:bg-muted/40"
                >
                  <div class="min-w-0 flex-1">
                    <div class="flex flex-wrap items-baseline gap-x-1.5">
                      <span class="text-[12px] font-medium text-foreground">{def.label}</span>
                      {#if def.scopeNote}
                        <span class="text-[10px] text-muted-foreground">· {def.scopeNote}</span>
                      {/if}
                    </div>
                    {#if def.description}
                      <p class="text-[10px] leading-tight text-muted-foreground">{def.description}</p>
                    {/if}
                  </div>
                  <div class="flex shrink-0 items-center gap-1 pt-px">
                    {#each tokensFor(def) as t}
                      <Kbd>{t}</Kbd>
                    {/each}
                  </div>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </div>
    </div>

    <footer
      class="flex h-10 items-center justify-between gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
    >
      <span class="flex items-center gap-1">
        <Kbd>Esc</Kbd>
        <span>Close</span>
      </span>
      <span class="text-[10px]">{shortcutDefs.length} shortcuts</span>
    </footer>
  </Dialog.Content>
</Dialog.Root>
