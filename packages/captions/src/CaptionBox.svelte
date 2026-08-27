<script lang="ts">
// Presentational caption renderer shared by the desktop editor preview and
// the web player overlay. It owns the LOOK (pill, typography, per-word colour,
// entrance) and nothing else: no store, no clock, no time mapping. The parent
// resolves which chunk is active and how far speech has progressed, then hands
// this component the words plus `spokenCount` / `activeIndex`.
//
// Styling is component-scoped CSS on purpose. Workspace packages that ship
// Tailwind utility classes must be registered in each app's `@source`, or the
// classes purge in release builds only; scoped CSS sidesteps that entirely.
import type { CaptionAnimation, CaptionStyle, TranscriptWord } from "./types";
import { breakIntoLines } from "./linebreak";
import { wordColor, wordScaled } from "./word-render";
import { withAlpha } from "./color";

let {
	words,
	style,
	anim,
	spokenCount,
	activeIndex,
	fontSize,
}: {
	words: TranscriptWord[];
	style: CaptionStyle;
	anim: CaptionAnimation;
	/** How many of `words` are spoken (progressive highlight). */
	spokenCount: number;
	/** Currently-spoken word index, -1 if none (active/scale emphasis). */
	activeIndex: number;
	/** CSS length for the caption font size, e.g. "3.8cqh" or "24px". The
	 *  parent decides the sizing strategy; em-based padding/radius scale off it. */
	fontSize: string;
} = $props();

const lines = $derived(breakIntoLines(words, style.maxCharsPerLine, style.maxLines));

const rootStyle = $derived.by(() => {
	const parts = [
		`font-size: ${fontSize}`,
		`font-family: ${style.fontFamily}`,
		`font-weight: ${style.fontWeight}`,
		`line-height: ${style.lineHeight}`,
		`letter-spacing: ${style.letterSpacing}em`,
		`text-transform: ${style.uppercase ? "uppercase" : "none"}`,
		`--rc-entrance-ms: ${anim.entranceMs}ms`,
	];
	if (style.outlineWidth > 0) {
		parts.push(
			`-webkit-text-stroke: ${style.outlineWidth / 100}em ${style.outlineColor}`,
			`paint-order: stroke fill`,
		);
	}
	if (style.background === "box") {
		parts.push(
			`background: ${withAlpha(style.backgroundColor, style.backgroundOpacity / 100)}`,
			`padding: ${style.boxPaddingYEm}em ${style.boxPaddingXEm}em`,
			// CSS clamps a radius larger than half the box to a stadium, which
			// matches geometry.pillBox's explicit clamp on the export side.
			`border-radius: ${style.boxRadiusEm}em`,
		);
	}
	return parts.join("; ");
});

const alignItems = $derived(
	style.align === "left" ? "flex-start" : style.align === "right" ? "flex-end" : "center",
);
</script>

<div
  class="rc-cap entrance-{anim.entrance}"
  class:soft={style.background === "soft"}
  style="{rootStyle}; align-items: {alignItems}; text-align: {style.align};"
>
  {#each lines as line, li (li)}
    <div class="rc-cap-line">
      {#each line as wi, k (wi)}{#if k > 0}{" "}{/if}<span
          class="rc-cap-word"
          class:scaled={wordScaled({ index: wi, activeIndex, wordCount: words.length, anim })}
          style:color={wordColor({
            index: wi,
            activeIndex,
            spokenCount,
            wordCount: words.length,
            style,
            anim,
          })}>{words[wi].text}</span
        >{/each}
    </div>
  {/each}
</div>

<style>
  .rc-cap {
    display: inline-flex;
    flex-direction: column;
    max-width: 100%;
    /* A word turning muted -> bright animates on colour only (paint), never a
       keyframe, so rapid retriggers and scrubbing retarget mid-flight. */
  }
  .rc-cap-line {
    white-space: pre;
    display: block;
  }
  .rc-cap-word {
    transition: color 120ms ease;
  }
  .rc-cap-word.scaled {
    display: inline-block;
    transform: scale(1.14);
    transition:
      color 120ms ease,
      transform 120ms ease;
  }
  .soft {
    text-shadow:
      0 1px 2px rgba(0, 0, 0, 0.9),
      0 0 6px rgba(0, 0, 0, 0.7);
  }

  /* Per-chunk entrance. The parent re-mounts this component on chunk change
     (via {#key}), which replays the animation. `none` gets no rule. The pop
     comes from scale(0.97)->1 + a strong ease-out, NOT a bouncy overshoot. */
  .entrance-fade {
    animation: rc-cap-fade var(--rc-entrance-ms, 125ms) ease-out both;
  }
  .entrance-pop {
    animation: rc-cap-pop var(--rc-entrance-ms, 125ms) cubic-bezier(0.23, 1, 0.32, 1) both;
  }
  .entrance-slide {
    animation: rc-cap-slide var(--rc-entrance-ms, 125ms) cubic-bezier(0.23, 1, 0.32, 1) both;
  }
  @keyframes rc-cap-fade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes rc-cap-pop {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
  @keyframes rc-cap-slide {
    from {
      opacity: 0;
      transform: translateY(0.25em) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  /* Reduced motion: keep the word visible and let colour still convey progress
     (comprehension), but drop all movement. */
  @media (prefers-reduced-motion: reduce) {
    .entrance-fade,
    .entrance-pop,
    .entrance-slide {
      animation: none;
    }
    .rc-cap-word.scaled {
      transform: none;
    }
  }
</style>
