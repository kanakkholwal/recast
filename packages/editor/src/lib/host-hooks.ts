/**
 * Optional hooks the host app can own but the editor must not require: product
 * analytics, the app-wide shortcut registry, and the export queue's rendering
 * flag. Each has a no-op default, so a host that installs nothing still gets a
 * working editor — the events go nowhere and the shortcuts stay unbound.
 */

export interface EditorAnalytics {
	capture(event: string, props?: Record<string, unknown>): void;
}

export interface ShortcutHost {
	/** Human-readable chord for an action id (e.g. "⌘S"), or "" when unbound. */
	chordLabel(id: string): string;
	/** Bind handlers for this editor's lifetime. Returns an unregister fn. */
	registerShortcutHandlers(handlers: Record<string, () => void>): () => void;
}

export interface ExportActivityHost {
	/** True while a browser export is compositing. The preview pauses its own
	 *  decode/render while set, so the two don't fight over the GPU. */
	readonly renderingInBrowser: boolean;
}

interface HostHooks {
	analytics: EditorAnalytics;
	shortcuts: ShortcutHost;
	exportActivity: ExportActivityHost;
}

const noop: HostHooks = {
	analytics: { capture: () => {} },
	shortcuts: { chordLabel: () => "", registerShortcutHandlers: () => () => {} },
	exportActivity: { renderingInBrowser: false },
};

let hooks: HostHooks = noop;

/** Install real implementations. Returns a restore fn so tests don't leak. */
export function setEditorHostHooks(next: Partial<HostHooks>): () => void {
	const previous = hooks;
	hooks = { ...hooks, ...next };
	return () => {
		hooks = previous;
	};
}

export const analytics: EditorAnalytics = {
	capture: (event, props) => hooks.analytics.capture(event, props),
};

export const chordLabel = (id: string): string => hooks.shortcuts.chordLabel(id);

export const registerShortcutHandlers = (handlers: Record<string, () => void>): (() => void) =>
	hooks.shortcuts.registerShortcutHandlers(handlers);

export const exportActivity: ExportActivityHost = {
	get renderingInBrowser() {
		return hooks.exportActivity.renderingInBrowser;
	},
};
