/**
 * Export cursor-sprite overlay: draws the rasterized SVG cursor as a textured
 * quad, matching the preview's DOM `<img>` placement (hotspot anchored at the
 * cursor sample, scaled about the hotspot). Dot-style cursors are drawn by the
 * main shader, so this pass only exists for sprite styles.
 */

import type { SvgCursorParams } from "../../components/editor/frame-params";
import type { WebGL2Backend } from "../../components/editor/webgl2-backend";
import type { RenderPass, RenderPassContext } from "../../components/editor/render-core";
import type { FrameParams } from "../../components/editor/frame-params";
import type { QuadRect } from "../../components/editor/overlay-quad";
import type { ExportOverlay, ExportOverlayFactory } from "./offscreen-export";

export type CursorState = "rest" | "press" | "rightPress" | "drag";

/** Which sprite variant to draw, matching the preview's stateKey selection. */
export function pickCursorState(c: SvgCursorParams): CursorState {
	if (!c.pressed) return "rest";
	if (c.dragging) return "drag";
	if (c.right) return "rightPress";
	return "press";
}

/** Sprite quad in canvas pixels: the hotspot lands on the cursor sample point and
 *  the sprite scales about it — the GL twin of the preview's translate+scale with
 *  `transform-origin` at the hotspot. `hot` is the hotspot in 0..1 sprite UV. */
export function cursorSpriteRect(
	c: SvgCursorParams,
	hot: readonly [number, number],
	canvasPxW: number,
	canvasPxH: number,
): QuadRect {
	const sx = canvasPxW / Math.max(1, c.compW);
	const sy = canvasPxH / Math.max(1, c.compH);
	const w = c.spritePx * sx * c.scale;
	const h = c.spritePx * sy * c.scale;
	return { x: c.canvasX * sx - hot[0] * w, y: c.canvasY * sy - hot[1] * h, w, h };
}

/** Rasterized sprites (as bitmaps) + per-state hotspots, from `rasterizeCursorSprites`. */
export interface CursorSpriteSources {
	rest: ImageBitmap;
	press: ImageBitmap;
	rightPress?: ImageBitmap;
	drag?: ImageBitmap;
	restHotspot: [number, number];
	pressHotspot: [number, number];
	rightPressHotspot?: [number, number];
	dragHotspot?: [number, number];
}

/** Build an export overlay factory for the cursor sprites. Uploads one texture
 *  per distinct state on create; falls back drag/rightPress → press → rest to
 *  mirror the preview's `resolveCursorDataUrl`. */
export function cursorOverlayFactory(sprites: CursorSpriteSources): ExportOverlayFactory {
	return (backend: WebGL2Backend): ExportOverlay => {
		const textures: WebGLTexture[] = [];
		const upload = (bmp: ImageBitmap): WebGLTexture => {
			const t = backend.createTextureFrom(bmp);
			bmp.close(); // consumed into the texture; not needed again
			textures.push(t);
			return t;
		};
		const restTex = upload(sprites.rest);
		const pressTex = sprites.press === sprites.rest ? restTex : upload(sprites.press);
		const rightTex = sprites.rightPress ? upload(sprites.rightPress) : pressTex;
		const dragTex = sprites.drag ? upload(sprites.drag) : pressTex;

		const byState: Record<CursorState, { tex: WebGLTexture; hot: [number, number] }> = {
			rest: { tex: restTex, hot: sprites.restHotspot },
			press: { tex: pressTex, hot: sprites.pressHotspot },
			rightPress: {
				tex: rightTex,
				hot: sprites.rightPressHotspot ?? sprites.pressHotspot,
			},
			drag: { tex: dragTex, hot: sprites.dragHotspot ?? sprites.pressHotspot },
		};

		const pass: RenderPass = {
			name: "cursor-sprite",
			render(be: WebGL2Backend, params: FrameParams, _ctx: RenderPassContext) {
				const c = params.svgCursor;
				if (!c || !c.visible || c.alpha <= 0) return;
				const [cw, ch] = params.uniforms.canvasSize;
				const { tex, hot } = byState[pickCursorState(c)];
				be.drawSprite(tex, cursorSpriteRect(c, hot, cw, ch), { alpha: c.alpha });
			},
		};

		return {
			pass,
			dispose() {
				for (const t of textures) backend.deleteTexture(t);
			},
		};
	};
}
