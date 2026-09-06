<script lang="ts">
// Presentational only: it owns the look, while the parent resolves which chunk is active. Scoped CSS, since utility classes from a workspace package purge unless registered in each app's @source.
import { withAlpha } from "./color";
import { breakIntoLines } from "./linebreak";
import type { CaptionAnimation, CaptionStyle, TranscriptWord } from "./types";
import { wordColor, wordScaled } from "./word-render";

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
			// CSS clamps a radius larger than half the box to a stadium, matching geometry.pillBox's export-side clamp.
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
    /* Muted to bright animates on colour only, never a keyframe, so rapid retriggers and scrubbing retarget mid-flight. */
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

  /* The parent re-mounts on chunk change via {#key}, replaying this; the pop is scale 0.97 to 1 with a strong ease-out, not an overshoot. */
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

  /* Reduced motion: keep the word visible and let colour still convey progress, but drop all movement. */
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
