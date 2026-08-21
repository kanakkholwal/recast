const REPO = "https://github.com/kanakkholwal/recast/blob/main/";

/** Rust paths are written module-relative in the docs, not from the repo root. */
const RUST_SRC = "apps/desktop/src-tauri/src/";

const RUST_DIRS = [
	"audio",
	"capture",
	"commands",
	"control",
	"cursor",
	"encoder",
	"mcp",
	"project",
	"recording",
	"render",
	"transcription",
];

/** Unambiguous files that sit directly in the Rust crate root. */
const RUST_ROOT_FILES = ["cli.rs", "ffmpeg.rs", "lib.rs"];

const SOURCE_EXTENSIONS = /\.(rs|ts|tsx|js|mjs|svelte|toml)$/;

/**
 * Repo path for a reference as written in a doc, or `null` when it is too
 * ambiguous to link.
 *
 * A bare `mod.rs` or `time-map.ts` names a dozen real files, so it stays plain
 * code. Only a path that resolves to exactly one file becomes a link.
 */
export function repoPath(reference: string): string | null {
	const path = reference.trim();
	if (!SOURCE_EXTENSIONS.test(path)) return null;
	if (path.includes("..") || path.startsWith("/")) return null;

	if (path.startsWith("apps/") || path.startsWith("packages/")) return path;
	if (RUST_ROOT_FILES.includes(path)) return RUST_SRC + path;

	const [head] = path.split("/");
	if (head !== path && RUST_DIRS.includes(head)) return RUST_SRC + path;

	return null;
}

/** Link to a reference on GitHub, or `null` when it should stay plain code. */
export function sourceUrl(reference: string): string | null {
	const path = repoPath(reference);
	return path === null ? null : REPO + path;
}
