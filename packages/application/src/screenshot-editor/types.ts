/** Types shared across the screenshot editor. Kept framework-free so the state
 * store, presets, and export code stay testable without a component. */

/** A loaded source image plus its intrinsic pixel size (needed for aspect and
 * export math). `src` is a data/blob/object URL usable directly in `<img>`. */
export interface EditorImage {
	src: string;
	width: number;
	height: number;
	/** Original file name (sans extension), used to seed the export filename. */
	name?: string;
}

/** How the stage backdrop is painted behind the screenshot. */
export type Background =
	| { kind: "solid"; color: string }
	| { kind: "gradient"; css: string }
	| { kind: "transparent" };

/** A named backdrop the user can pick from the swatch grid. */
export interface BackgroundPreset {
	id: string;
	label: string;
	background: Background;
	/** Small CSS value used to paint the swatch (mirrors the backdrop). */
	swatch: string;
}

/** Frame treatment applied to the screenshot itself (not the backdrop). */
export interface Frame {
	/** Space between the screenshot and the stage edge, as a % of the stage
	 * width, so padding is symmetric across aspect ratios. */
	padding: number;
	/** Corner radius in CSS pixels (stage space). */
	radius: number;
	/** Border around the screenshot; width 0 hides it. */
	border: { width: number; color: string };
}

/** One-click screenshot frame looks (mirrors the clone's `imageStylePreset`).
 * `default` = no wrapper; the rest wrap the shot in a tinted/solid padded card. */
export type ImageStylePreset =
	| "default"
	| "glass-light"
	| "glass-dark"
	| "outline"
	| "border-light"
	| "border-dark";

/** The active style-frame wrapper: which preset, plus its two live controls. */
export interface ImageStyle {
	preset: ImageStylePreset;
	/** Wrapper padding as a percent of the shot width (concentric card). */
	padding: number;
	/** Tint alpha for the glass/outline backgrounds (ignored by solid borders). */
	opacity: number;
}

/** Named drop-shadow strengths (mirrors the clone's `shadowPreset`). */
export type ShadowPreset = "none" | "hug" | "soft" | "strong";

/** CSS filter adjustments applied to the screenshot itself. Percent-based
 * values match the clone's `imageFilters` (100 = unchanged); `hueRotate` is in
 * degrees, `blur` in pixels. Composed into a single CSS `filter` string. */
export interface ImageFilters {
	brightness: number;
	contrast: number;
	saturate: number;
	grayscale: number;
	sepia: number;
	hueRotate: number;
	invert: number;
	blur: number;
}

/** Drop shadow beneath the framed screenshot. Composed into a CSS box-shadow. */
export interface Shadow {
	x: number;
	y: number;
	blur: number;
	spread: number;
	/** 0..1 shadow alpha; 0 hides the shadow. */
	opacity: number;
	color: string;
}

/** Chrome wrapped around the screenshot to frame it as an app/browser window
 * or a device. All variants are pure CSS on the DOM stage, so they export
 * identically. Browser kinds fill the frame; device kinds impose their own
 * aspect ratio. */
export type MockupKind = "none" | "window" | "safari" | "chrome" | "phone" | "tablet";

/** Light/dark browser chrome, independent of the app's own theme. */
export type MockupTheme = "light" | "dark";

export interface Mockup {
	kind: MockupKind;
	theme: MockupTheme;
	/** Address-bar text for the browser variants (safari/chrome). */
	url: string;
}

/** 3D transform applied to the framed screenshot (pure CSS perspective). Lower
 * `perspective` = stronger depth; angles in degrees; `scale` 0.5..1.5;
 * `translateX/Y` shift the shot as a percent of its own size (matches the
 * reference `perspective3D.translateX/Y`, range -10..10). */
export interface Transform3D {
	perspective: number;
	rotateX: number;
	rotateY: number;
	rotateZ: number;
	scale: number;
	translateX: number;
	translateY: number;
}

/** A named 3D preset for one-click tilts. */
export interface PerspectivePreset {
	id: string;
	label: string;
	transform: Transform3D;
}

/** A selectable output aspect ratio. `ratio` is width/height; `null` means
 * "match the screenshot" (no letterboxing). */
export interface AspectPreset {
	id: string;
	label: string;
	ratio: number | null;
}

/** Overlays sit in an absolute layer over the stage. Position is in percent of
 * the stage so overlays stay put as the preview resizes and on export. */
export interface BaseOverlay {
	id: string;
	/** Center position, 0..100 percent of stage width/height. */
	x: number;
	y: number;
	rotation: number;
	/** 0..1 overlay opacity. */
	opacity: number;
	/** Hidden overlays render neither in the preview nor an export. */
	isVisible: boolean;
}

export type TextAlign = "left" | "center" | "right";
export type TextOrientation = "horizontal" | "vertical";

/** Optional drop shadow on a text overlay (CSS `text-shadow`). */
export interface TextShadow {
	enabled: boolean;
	color: string;
	blur: number;
	offsetX: number;
	offsetY: number;
}

export interface TextOverlay extends BaseOverlay {
	type: "text";
	text: string;
	fontSize: number;
	/** Font id from the font library (resolved to a CSS stack via `fontCss`). */
	fontFamily: string;
	/** CSS weight token ("normal"/"bold"/"100".."900"); ids drive availability. */
	fontWeight: string;
	color: string;
	align: TextAlign;
	orientation: TextOrientation;
	shadow: TextShadow;
}

/** Shape/arrow annotations drawn over the stage. */
export type ShapeKind = "rectangle" | "ellipse" | "arrow" | "line";

export interface ShapeOverlay extends BaseOverlay {
	type: "shape";
	shape: ShapeKind;
	/** Size in percent of the stage (arrows/lines use it as the vector to x2,y2). */
	w: number;
	h: number;
	strokeColor: string;
	fillColor: string;
	strokeWidth: number;
	/** When true, `fillColor` paints the interior (rectangle/ellipse only). */
	filled: boolean;
}

/** An image/sticker overlay (uploaded logo, emoji, or a light/shadow overlay). */
export interface ImageOverlay extends BaseOverlay {
	type: "image";
	src: string;
	/** Width as a percent of the stage; height follows the image's aspect. */
	size: number;
	/** Gaussian blur in px (for soft light/shadow overlays). */
	blur: number;
	flipX: boolean;
	flipY: boolean;
	/** front sits over the screenshot; back sits behind it. */
	layer: "front" | "back";
	/** True for a user upload (vs a built-in overlay asset). */
	isCustom: boolean;
	/** "shadow" marks the single built-in light/shadow overlay (singleton, so a
	 * new pick replaces it rather than stacking). Absent for normal stickers. */
	role?: "shadow";
}

/** A rectangular redaction region that blurs whatever sits under it. */
export interface BlurOverlay extends BaseOverlay {
	type: "blur";
	/** Size in percent of the stage. */
	w: number;
	h: number;
	/** Backdrop blur radius in px. */
	blurAmount: number;
}

export type Overlay = TextOverlay | ShapeOverlay | ImageOverlay | BlurOverlay;

/** A one-click design: applies a coordinated background + frame + shadow +
 * mockup + 3D, leaving the image and overlays untouched. */
export interface Template {
	id: string;
	label: string;
	backgroundId: string;
	background: Background;
	padding: number;
	radius: number;
	shadow: Shadow;
	mockup: Mockup;
	transform: Transform3D;
	/** A CSS `background` value for the template's swatch. */
	swatch: string;
}

/** The design (everything except the image), as serialized for history,
 * custom presets, and the autosave draft. */
export interface DesignObject {
	background: Background;
	backgroundId: string;
	frame: Frame;
	shadow: Shadow;
	shadowPreset: ShadowPreset;
	imageStyle: ImageStyle;
	mockup: Mockup;
	transform: Transform3D;
	aspectId: string;
	overlays: Overlay[];
	filters: ImageFilters;
	imageScale: number;
	imageOpacity: number;
	backgroundBlur: number;
	backgroundNoise: number;
	canvasRadius: number;
	keyframes?: import("./animation").KeyframeEntry[];
}

/** A full editor snapshot (design + image + export prefs) for the autosave draft. */
export interface EditorSnapshot {
	v: number;
	image: EditorImage | null;
	slides?: EditorImage[];
	activeSlide?: number;
	design: DesignObject;
	exportFormat: ExportFormat;
	exportScale: number;
	exportQuality: number;
}

/** A user-saved look (design only), stored in localStorage. */
export interface CustomPreset {
	id: string;
	name: string;
	createdAt: number;
	design: DesignObject;
}

export type ExportFormat = "png" | "jpeg" | "webp";

export interface ExportSpec {
	format: ExportFormat;
	/** Device-pixel multiplier for the snapshot (1..4). */
	scale: number;
	/** Encoder quality 0..1 for the lossy formats (jpeg/webp); ignored for png. */
	quality?: number;
}
