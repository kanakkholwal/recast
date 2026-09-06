import { setMode, toggleMode } from "mode-watcher";

type Mode = "light" | "dark" | "system";

const STYLE_ID = "recast-theme-vt";

// Scoped by `html[data-recast-vt]` so it only bites during a theme toggle: the circle reveal owns root, and the chrome's own view-transition-names are nulled (!important beats their inline styles) so they fold into that reveal instead of cross-fading ahead of it.
const CSS = `
html[data-recast-vt="circle-blur"] [data-recast-topnav],
html[data-recast-vt="circle-blur"] [data-recast-topnav] *,
html[data-recast-vt="circle-blur"] [data-recast-titlebar],
html[data-recast-vt="circle-blur"] [data-sidebar="sidebar"] {
  view-transition-name: none !important;
}
html[data-recast-vt="circle-blur"]::view-transition-old(root) { animation: none; }
html[data-recast-vt="circle-blur"]::view-transition-new(root) {
  animation: recast-vt-circle-blur 640ms cubic-bezier(0.4, 0, 0.2, 1);
}
@keyframes recast-vt-circle-blur {
  from { clip-path: circle(0% at var(--recast-vt-origin, 50% 50%)); filter: blur(7px); }
  to   { clip-path: circle(150% at var(--recast-vt-origin, 50% 50%)); filter: blur(0px); }
}`;

function ensureStyle() {
	if (typeof document === "undefined" || document.getElementById(STYLE_ID)) return;
	const el = document.createElement("style");
	el.id = STYLE_ID;
	el.textContent = CSS;
	document.head.appendChild(el);
}

function prefersReduced() {
	return (
		typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches
	);
}

type VTDocument = Document & {
	startViewTransition?: (cb: () => void) => { finished: Promise<void> };
};

// Runs `apply` inside a circle-blur reveal from `origin` (screen px, default centre), or applies it instantly when transitions are unavailable or reduced.
function runThemeTransition(apply: () => void, origin?: { x: number; y: number }): void {
	const doc = document as VTDocument;
	if (prefersReduced() || typeof doc.startViewTransition !== "function") {
		apply();
		return;
	}
	ensureStyle();
	const root = document.documentElement;
	const x = origin ? (origin.x / window.innerWidth) * 100 : 50;
	const y = origin ? (origin.y / window.innerHeight) * 100 : 50;
	root.style.setProperty("--recast-vt-origin", `${x}% ${y}%`);
	root.dataset.recastVt = "circle-blur";
	const transition = doc.startViewTransition(() => apply());
	transition.finished.finally(() => {
		delete root.dataset.recastVt;
	});
}

/** Flip light/dark with a circle-blur reveal from `origin` (screen px). */
export function toggleModeCircleBlur(origin?: { x: number; y: number }): void {
	runThemeTransition(() => toggleMode(), origin);
}

/** Set the theme with the same circle-blur reveal, for sources without a pointer (command palette, settings). */
export function setModeCircleBlur(mode: Mode, origin?: { x: number; y: number }): void {
	runThemeTransition(() => setMode(mode), origin);
}
