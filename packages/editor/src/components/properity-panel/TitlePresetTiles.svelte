<script lang="ts">
import { TITLE_PRESETS, type TitlePreset } from "../../lib/annotations/title-presets";

// Each tile lays the preset's own text out at its real UV position, size and
// weight, so you pick by look instead of guessing what "Lower third" means.
// `cqh` because fontSize and glow blur are fractions of frame height.
interface Props {
	oninsert: (preset: TitlePreset) => void;
}

let { oninsert }: Props = $props();
</script>

<div class="grid grid-cols-2 gap-1.5">
	{#each TITLE_PRESETS as preset (preset.id)}
		{@const k = preset.build()}
		<button
			type="button"
			onclick={() => oninsert(preset)}
			title="Insert {preset.label.toLowerCase()}"
			class="group flex flex-col gap-1 rounded-lg border border-border/50 p-1 text-left transition-colors hover:border-border hover:bg-muted/40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
		>
			<span
				class="relative block aspect-video max-h-20 w-full overflow-hidden rounded-md bg-neutral-950"
				style="container-type: size;"
			>
				<span
					class="absolute block truncate text-white"
					style="left: {k.x * 100}%; top: {k.y * 100}%; width: {k.w * 100}%;
					       font-family: {k.fontFamily}; font-size: {k.fontSize * 100}cqh;
					       font-weight: {k.fontWeight}; line-height: {k.lineHeight};
					       text-align: {k.align}; color: {k.color};
					       text-shadow: 0 0 {preset.glow.blur * 100}cqh rgba(0, 0, 0, {preset.glow
						.opacity});"
				>
					{k.content}
				</span>
			</span>
			<span class="px-0.5 text-[11px] text-muted-foreground group-hover:text-foreground">
				{preset.label}
			</span>
		</button>
	{/each}
</div>
