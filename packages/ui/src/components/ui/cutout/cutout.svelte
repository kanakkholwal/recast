<script lang="ts" module>
export type CutoutCorner = "tl" | "tr" | "bl" | "br";
/** Token surface the notch blends into (must match the area behind the corner). */
export type CutoutSurface = "background" | "card" | "muted" | "popover" | "inherit" | "custom";
</script>

<script lang="ts">
  /**
   * Corner chip with an inverted ("carved out") radius. Anchors to one corner
   * of a `relative` parent and uses two `currentColor` box-shadow pseudo-corners
   * to bend the surrounding surface into a concave notch — so the chip reads as
   * punched out of the parent rather than laid on top.
   *
   * `surface` sets both the chip fill and the notch colour, so it MUST match the
   * token of whatever sits behind that corner (page = `background`, inside a
   * card = `card`). Inner content sets its own text colour — the wrapper's text
   * colour is reserved to drive the notch.
   *
   * Padding caveat: each notch reaches up to `radius` px into the chip on its
   * two notched sides (e.g. a `bl` corner notches the TOP and RIGHT). Keep
   * padding on those sides ≥ `radius` or the surface-coloured notch will paint
   * over the content.
   */
  import { cn } from "@recast/ui/utils";
  import type { Snippet } from "svelte";

  let {
    corner = "tr",
    surface = "background",
    radius = 16,
    class: className,
    children,
    ...rest
  }: {
    corner?: CutoutCorner;
    surface?: CutoutSurface;
    radius?: number;
    class?: string;
    children: Snippet;
  } & Record<string, unknown> = $props();

  const surfaceClass: Record<CutoutSurface, string> = {
    background: "bg-background text-background",
    card: "bg-card text-card",
    muted: "bg-muted text-muted",
    popover: "bg-popover text-popover",
    inherit: "bg-inherit text-current",
    custom: "bg-[var(--cut-bg)] text-[var(--cut-text)]",
  };
  const anchorClass: Record<CutoutCorner, string> = {
    tr: "right-0 top-0 rounded-bl-[var(--cut)]",
    tl: "left-0 top-0 rounded-br-[var(--cut)]",
    br: "right-0 bottom-0 rounded-tl-[var(--cut)]",
    bl: "left-0 bottom-0 rounded-tr-[var(--cut)]",
  };
</script>

<div
  data-slot="cutout"
  data-corner={corner}
  style={`--cut:${radius}px`}
  class={cn("cutout absolute z-10", anchorClass[corner], surfaceClass[surface], className)}
  {...rest}
>
  {@render children()}
</div>

<style>
  .cutout::before,
  .cutout::after {
    content: "";
    position: absolute;
    width: var(--cut);
    height: var(--cut);
    background: transparent;
    pointer-events: none;
    transition: box-shadow 0.3s ease;
  }

  .cutout[data-corner="tr"]::before {
    top: 0;
    left: calc(var(--cut) * -1);
    border-top-right-radius: var(--cut);
    box-shadow: calc(var(--cut) / 2) calc(var(--cut) / -2) 0 calc(var(--cut) / 2) currentColor;
  }
  .cutout[data-corner="tr"]::after {
    bottom: calc(var(--cut) * -1);
    right: 0;
    border-top-right-radius: var(--cut);
    box-shadow: calc(var(--cut) / 2) calc(var(--cut) / -2) 0 calc(var(--cut) / 2) currentColor;
  }

  .cutout[data-corner="tl"]::before {
    top: 0;
    right: calc(var(--cut) * -1);
    border-top-left-radius: var(--cut);
    box-shadow: calc(var(--cut) / -2) calc(var(--cut) / -2) 0 calc(var(--cut) / 2) currentColor;
  }
  .cutout[data-corner="tl"]::after {
    bottom: calc(var(--cut) * -1);
    left: 0;
    border-top-left-radius: var(--cut);
    box-shadow: calc(var(--cut) / -2) calc(var(--cut) / -2) 0 calc(var(--cut) / 2) currentColor;
  }

  .cutout[data-corner="bl"]::before {
    top: calc(var(--cut) * -1);
    left: 0;
    border-bottom-left-radius: var(--cut);
    box-shadow: calc(var(--cut) / -2) calc(var(--cut) / 2) 0 calc(var(--cut) / 2) currentColor;
  }
  .cutout[data-corner="bl"]::after {
    bottom: 0;
    right: calc(var(--cut) * -1);
    border-bottom-left-radius: var(--cut);
    box-shadow: calc(var(--cut) / -2) calc(var(--cut) / 2) 0 calc(var(--cut) / 2) currentColor;
  }

  .cutout[data-corner="br"]::before {
    top: calc(var(--cut) * -1);
    right: 0;
    border-bottom-right-radius: var(--cut);
    box-shadow: calc(var(--cut) / 2) calc(var(--cut) / 2) 0 calc(var(--cut) / 2) currentColor;
  }
  .cutout[data-corner="br"]::after {
    bottom: 0;
    left: calc(var(--cut) * -1);
    border-bottom-right-radius: var(--cut);
    box-shadow: calc(var(--cut) / 2) calc(var(--cut) / 2) 0 calc(var(--cut) / 2) currentColor;
  }
</style>
