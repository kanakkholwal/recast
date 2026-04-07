<script lang="ts">
  import { WindowControls } from "@tauri-controls-v2/svelte";
  import type { Snippet } from "svelte";

  interface Props {
    children?: Snippet;
  }

  let { children }: Props = $props();
</script>

<div
  class="h-10 flex items-center border-b border-border bg-background shrink-0 select-none"
  data-tauri-drag-region
>
  <div class="flex-1 flex items-center min-w-0 h-full" data-tauri-drag-region>
    {#if children}
      {@render children()}
    {/if}
  </div>

  <div class="shrink-0 ml-auto">
    <WindowControls />
  </div>
</div>
