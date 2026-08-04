<script lang="ts">
import { type CaptionStyle, resolveCaptionAnimation, type TranscriptWord } from "@recast/captions";
import CaptionBox from "@recast/captions/box";
import { cn } from "@recast/ui/utils";
import { ensureFontLoaded } from "../../lib/fonts/font-options";

// A grid of real previews instead of a searchable dropdown. Six themes did not
// need a command palette, and the old row (44px swatch + name + description)
// asked the user to read three things to picture one. Each tile renders through
// CaptionBox, the same component the preview and the player use, so what you
// see is what the export burns in.
interface ThemeOption {
	id: string;
	label: string;
	value: Omit<CaptionStyle, "enabled">;
}

interface Props {
	themes: ThemeOption[];
	/** Id of the theme matching the current style, or null once tweaked. */
	activeId: string | null;
	onSelect: (value: Omit<CaptionStyle, "enabled">) => void;
}

let { themes, activeId, onSelect }: Props = $props();

// One spoken, one unspoken: the gap between them is what the progressive
// highlight actually looks like, which no swatch could show. Two words, because
// a display face at tile scale runs out of room fast.
const SAMPLE: TranscriptWord[] = [
	{ start: 0, end: 1, text: "Ship" },
	{ start: 1, end: 2, text: "it" },
];

$effect(() => {
	for (const t of themes) ensureFontLoaded(t.value.fontFamily, t.value.fontWeight);
});

// One size for every tile. Scaling by each theme's own `fontSizePct` was
// technically honest and practically wrong: at tile scale the small themes were
// unreadable and the display face overflowed. The tile's job is to show the
// look (face, pill, colours, highlight); size is what the Font size slider is
// for, and it is the same slider whichever theme you land on.
const PREVIEW_FONT_SIZE = "13cqh";
</script>

<div role="radiogroup" aria-label="Caption theme" class="grid grid-cols-2 gap-1.5">
	{#each themes as theme (theme.id)}
		{@const selected = theme.id === activeId}
		<button
			type="button"
			role="radio"
			aria-checked={selected}
			onclick={() => onSelect(theme.value)}
			class={cn(
				"group flex flex-col gap-1 rounded-lg border p-1 text-left transition-colors",
				"focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
				selected
					? "border-foreground/30 bg-foreground/[0.06]"
					: "border-border/50 hover:border-border hover:bg-muted/40",
			)}
		>
			<span
				class="relative grid aspect-video w-full place-items-center overflow-hidden rounded-md bg-[#101014]"
				style="container-type: size;"
			>
				<CaptionBox
					words={SAMPLE}
					style={{ enabled: true, ...theme.value }}
					anim={resolveCaptionAnimation(theme.value.animation)}
					spokenCount={1}
					activeIndex={0}
					fontSize={PREVIEW_FONT_SIZE}
				/>
			</span>
			<span
				class={cn(
					"px-0.5 text-[11px]",
					selected ? "font-medium text-foreground" : "text-muted-foreground",
				)}
			>
				{theme.label}
			</span>
		</button>
	{/each}
</div>
