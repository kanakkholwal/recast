<script lang="ts">
import type { IconComponent } from "@recast/icons";
import type { Snippet } from "svelte";
import { fade } from "svelte/transition";

interface Props {
	icon: IconComponent;
	title: string;
	description: string;
	/** Call to action; omitted when the state speaks for itself. */
	action?: Snippet;
}

let { icon: Icon, title, description, action }: Props = $props();
</script>

<div
  in:fade={{ duration: 200 }}
  class="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border/60 bg-card/40 p-12 text-center"
>
  <div class="flex size-12 items-center justify-center rounded-xl bg-foreground/5 text-muted-foreground">
    <Icon class="size-5" />
  </div>
  <div>
    <p class="text-[14px] font-semibold text-foreground">{title}</p>
    <p class="mt-1 text-[11.5px] text-muted-foreground">{description}</p>
  </div>
  {@render action?.()}
</div>
