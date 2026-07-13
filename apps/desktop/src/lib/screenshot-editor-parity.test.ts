import { describe, expect, it } from "vitest";
import {
  clipTime,
  DEFAULT_FILTERS,
  DEFAULT_FRAME,
  DEFAULT_SHADOW,
  DEFAULT_STYLE,
  SHADOW_PRESETS,
  STYLE_PRESETS,
} from "@recast/application/screenshot-editor/defaults";
import {
  borderCss,
  filtersCss,
  shadowCss,
  styleFrameBackground,
  transformCss,
} from "@recast/application/screenshot-editor/render";

// Parity fixtures transcribed from the screenshot-studio reference app. These
// are the CLONE's values, written out by hand so a drift on our side fails here
// rather than silently changing the look.
//   defaults      -> lib/store/index.ts:717-760
//   shadowMap     -> lib/store/index.ts:1237-1242
//   borderMap     -> lib/store/index.ts:1221-1228
const CLONE_SHADOW_MAP = {
  none: { blur: 0, offsetX: 0, offsetY: 0, spread: 0, opacity: 0 },
  hug: { blur: 10, offsetX: 0, offsetY: 2, spread: 0, opacity: 0.25 },
  soft: { blur: 30, offsetX: 0, offsetY: 12, spread: 5, opacity: 0.5 },
  strong: { blur: 60, offsetX: 0, offsetY: 24, spread: 10, opacity: 0.8 },
} as const;

const CLONE_BORDER_MAP = {
  "glass-light": { opacity: 0.25, padding: 1 },
  "glass-dark": { opacity: 0.7, padding: 1 },
  outline: { opacity: 0.35, padding: 0.5 },
  "border-light": { padding: 1 },
  "border-dark": { padding: 1 },
} as const;

describe("shadow presets match the clone's shadowMap", () => {
  for (const [name, clone] of Object.entries(CLONE_SHADOW_MAP)) {
    it(`${name} maps to the clone's geometry`, () => {
      const ours = SHADOW_PRESETS[name as keyof typeof SHADOW_PRESETS];
      expect(ours.blur).toBe(clone.blur);
      expect(ours.x).toBe(clone.offsetX);
      expect(ours.y).toBe(clone.offsetY);
      expect(ours.spread).toBe(clone.spread);
      expect(ours.opacity).toBe(clone.opacity);
    });
  }
});

describe("style presets match the clone's borderMap", () => {
  for (const [name, clone] of Object.entries(CLONE_BORDER_MAP)) {
    it(`${name} seeds the clone's padding/opacity`, () => {
      const ours = STYLE_PRESETS[name as keyof typeof STYLE_PRESETS];
      expect(ours.padding).toBe(clone.padding);
      if ("opacity" in clone) expect(ours.opacity).toBe(clone.opacity);
    });
  }
});

describe("editor defaults match the clone", () => {
  it("borderRadius defaults to 10", () => {
    expect(DEFAULT_FRAME.radius).toBe(10);
  });

  it("the default shadow is the 'soft' preset", () => {
    expect(DEFAULT_SHADOW).toEqual(SHADOW_PRESETS.soft);
  });

  it("the default style frame is 'default'", () => {
    expect(DEFAULT_STYLE.preset).toBe("default");
  });

  it("image filters start neutral", () => {
    expect(DEFAULT_FILTERS).toEqual({
      brightness: 100,
      contrast: 100,
      saturate: 100,
      grayscale: 0,
      sepia: 0,
      hueRotate: 0,
      invert: 0,
      blur: 0,
    });
  });
});

describe("filtersCss", () => {
  it("emits none when every adjustment is neutral", () => {
    expect(filtersCss(DEFAULT_FILTERS)).toBe("none");
  });

  it("omits neutral channels and keeps the clone's order", () => {
    expect(filtersCss({ ...DEFAULT_FILTERS, contrast: 120, blur: 2 })).toBe(
      "contrast(120%) blur(2px)",
    );
  });

  it("uses degrees for hue and pixels for blur", () => {
    expect(filtersCss({ ...DEFAULT_FILTERS, hueRotate: 90 })).toBe("hue-rotate(90deg)");
  });
});

describe("styleFrameBackground", () => {
  it("tints glass-light with the live opacity", () => {
    expect(styleFrameBackground({ preset: "glass-light", padding: 1, opacity: 0.25 })).toBe(
      "rgba(255, 255, 255, 0.25)",
    );
  });

  it("tints glass-dark with the live opacity", () => {
    expect(styleFrameBackground({ preset: "glass-dark", padding: 1, opacity: 0.7 })).toBe(
      "rgba(0, 0, 0, 0.7)",
    );
  });

  it("keeps the solid borders opaque regardless of opacity", () => {
    expect(styleFrameBackground({ preset: "border-dark", padding: 1, opacity: 0.1 })).toBe(
      "rgb(26, 26, 26)",
    );
  });

  it("paints nothing for the default preset", () => {
    expect(styleFrameBackground(DEFAULT_STYLE)).toBe("transparent");
  });
});

describe("shadow and border composition", () => {
  it("hides the shadow at zero opacity", () => {
    expect(shadowCss(SHADOW_PRESETS.none)).toBe("none");
  });

  it("composes the soft preset into a box-shadow", () => {
    expect(shadowCss(SHADOW_PRESETS.soft)).toBe("0px 12px 30px 5px rgba(0, 0, 0, 0.5)");
  });

  it("hides a zero-width border", () => {
    expect(borderCss({ width: 0, color: "#fff" })).toBe("none");
  });

  it("composes the 3D transform", () => {
    expect(
      transformCss({ perspective: 1600, rotateX: 10, rotateY: -5, rotateZ: 0, scale: 1.1 }),
    ).toBe("rotateX(10deg) rotateY(-5deg) rotateZ(0deg) scale(1.1)");
  });
});

describe("clipTime maps the timeline playhead into preset time", () => {
  // A 2s preset placed at 1s on the track, stretched to 4s.
  const START = 1000;
  const LENGTH = 4000;
  const PRESET = 2000;

  it("holds the first frame before the clip starts", () => {
    expect(clipTime(0, START, LENGTH, PRESET)).toBe(0);
  });

  it("starts the preset exactly at the clip start", () => {
    expect(clipTime(START, START, LENGTH, PRESET)).toBe(0);
  });

  it("stretches the motion across the clip length", () => {
    // Halfway through a 4s clip is halfway through the 2s preset.
    expect(clipTime(START + LENGTH / 2, START, LENGTH, PRESET)).toBe(PRESET / 2);
  });

  it("holds the last frame after the clip ends", () => {
    expect(clipTime(START + LENGTH + 5000, START, LENGTH, PRESET)).toBe(PRESET);
  });

  it("is safe when the clip has no length", () => {
    expect(clipTime(500, 0, 0, PRESET)).toBe(0);
  });
});
