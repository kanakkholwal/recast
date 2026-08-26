<script lang="ts">
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

let {
	title,
	subtitle,
	actions,
	filters,
	detail,
	footer,
	children,
	class: className,
	contentClass,
	detailClass,
}: {
	title: string;
	subtitle?: string;
	actions?: Snippet;
	filters?: Snippet;
	detail?: Snippet;
	footer?: Snippet;
	children: Snippet;
	class?: string;
	contentClass?: string;
	detailClass?: string;
} = $props();
</script>

<div class={cn("flex h-full min-h-0 flex-col", className)}>
  <header
    class="flex h-14 shrink-0 items-center gap-3 border-b border-border/60 px-5"
    data-tauri-drag-region
  >
    <div class="min-w-0 flex-1">
      <h1 class="truncate text-[15px] font-semibold tracking-tight text-foreground">
        {title}
      </h1>
      {#if subtitle}
        <p class="truncate text-[11.5px] leading-tight text-muted-foreground/80">
          {subtitle}
        </p>
      {/if}
    </div>
    {#if actions}
      <div class="flex shrink-0 items-center gap-2">
        {@render actions()}
      </div>
    {/if}
  </header>

  {#if filters}
    <div
      class="flex shrink-0 flex-wrap items-center gap-2 border-b border-border/40 px-5 py-2.5"
    >
      {@render filters()}
    </div>
  {/if}

  <div class="flex min-h-0 flex-1">
    <div class={cn("min-w-0 flex-1 overflow-y-auto no-scrollbar", contentClass ?? "px-5 py-5")}>
      {@render children()}
    </div>
    {#if detail}
      <aside
        class={cn(
          "hidden w-[340px] shrink-0 overflow-y-auto border-l border-border/60 no-scrollbar lg:block",
          detailClass ?? "p-5",
        )}
      >
        {@render detail()}
      </aside>
    {/if}
  </div>

  {#if footer}
    <div class="shrink-0 border-t border-border/60">
      {@render footer()}
    </div>
  {/if}
</div>
