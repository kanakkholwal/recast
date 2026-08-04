// Shared annotation palette constants, centralised so the main panel and the
// appearance sub-panel can't drift out of sync.

/** Quick stroke / text color swatches. Mirrors the cursor highlight palette. */
export const STROKE_SWATCHES = [
	"#3b82f6",
	"#ef4444",
	"#22c55e",
	"#f59e0b",
	"#a855f7",
	"#ec4899",
	"#06b6d4",
	"#ffffff",
];

/** Quick fill swatches (translucent so the recording shows through). */
export const FILL_SWATCHES = [
	"transparent",
	"rgba(59,130,246,0.20)",
	"rgba(239,68,68,0.20)",
	"rgba(34,197,94,0.20)",
	"rgba(245,158,11,0.20)",
	"rgba(168,85,247,0.20)",
	"rgba(0,0,0,0.35)",
	"rgba(255,255,255,0.20)",
];

/**
 * Curated text-overlay font whitelist. All variable fonts are already loaded
 * via @fontsource-variable/* in app.css, plus generic system fallbacks.
 */
export const FONT_FAMILIES = [
	{ value: "'Geist Variable', system-ui, sans-serif", label: "Geist" },
	{ value: "'Geist Mono Variable', ui-monospace, monospace", label: "Geist Mono" },
	{ value: "'Google Sans Variable', system-ui, sans-serif", label: "Google Sans" },
	{ value: "system-ui, sans-serif", label: "System" },
	{ value: "ui-serif, Georgia, serif", label: "Serif" },
	{ value: "ui-monospace, monospace", label: "Monospace" },
];

export const FONT_WEIGHTS: {
	value: 400 | 500 | 600 | 700;
	label: string;
	title: string;
}[] = [
	{ value: 400, label: "R", title: "Regular" },
	{ value: 500, label: "M", title: "Medium" },
	{ value: 600, label: "SB", title: "Semibold" },
	{ value: 700, label: "B", title: "Bold" },
];
