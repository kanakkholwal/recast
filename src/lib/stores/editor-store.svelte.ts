/**
 * Editor Store — Central reactive state for the video editor.
 * Uses Svelte 5 runes ($state) for granular reactivity.
 */

export type BackgroundType = 'wallpaper' | 'image' | 'color' | 'gradient';


export interface WallpaperOption {
	src: string;
	label: string;
}

export interface ZoomRegion {
	id: string;
	start: number; // seconds
	end: number; // seconds
	scale: number; // 1.0 - 3.0
}

export interface CursorSettings {
	enabled: boolean;
	size: number; // 1-5 scale
	smoothing: number; // 0-100
	highlightClicks: boolean;
	highlightColor: string;
	highlightOpacity: number; // 0-100
	hideWhenIdle: boolean;
	idleTimeout: number; // seconds
}

export interface VideoMetadata {
	duration: number;
	width: number;
	height: number;
	fps: number;
	codec: string;
	sizeBytes: number;
}

export type ExportFormat = 'mp4' | 'gif' | 'webm';

export type LayoutMode = 'auto' | 'crop';

export type EditorWindowBehavior = 'navigate' | 'new-window';

export const WALLPAPERS: WallpaperOption[] = Array.from({ length: 22 }, (_, i) => ({
	src: `/wallpapers/wallpaper${i + 1}.png`,
	label: `Wallpaper ${i + 1}`,
}));

export const GRADIENT_PRESETS = [
	{ label: 'Sunset', value: 'linear-gradient(135deg, #f093fb 0%, #f5576c 100%)' },
	{ label: 'Ocean', value: 'linear-gradient(135deg, #4facfe 0%, #00f2fe 100%)' },
	{ label: 'Forest', value: 'linear-gradient(135deg, #43e97b 0%, #38f9d7 100%)' },
	{ label: 'Lavender', value: 'linear-gradient(135deg, #a18cd1 0%, #fbc2eb 100%)' },
	{ label: 'Midnight', value: 'linear-gradient(135deg, #0c3483 0%, #a2b6df 100%)' },
	{ label: 'Ember', value: 'linear-gradient(135deg, #f83600 0%, #f9d423 100%)' },
];

export const COLOR_PRESETS = [
	'#000000', '#1a1a2e', '#16213e', '#0f3460',
	'#533483', '#e94560', '#f38181', '#fce38a',
	'#eaffd0', '#95e1d3', '#ffffff', '#f5f5f5',
];

function generateId(): string {
	return Math.random().toString(36).substring(2, 9);
}

/**
 * Creates an editor store instance.
 * Call once per editor page mount, or use a singleton.
 */
export function createEditorStore() {
	// Video source
	let videoPath = $state('');
	let metadata = $state<VideoMetadata | null>(null);
	let thumbnailStrip = $state<string[]>([]);

	// Playback
	let currentTime = $state(0);
	let isPlaying = $state(false);

	// Trim
	let trimStart = $state(0);
	let trimEnd = $state(0); // will be set to duration on load

	// Background
	let backgroundType = $state<BackgroundType>('wallpaper');
	let backgroundValue = $state(WALLPAPERS[0].src);
	let backgroundBlur = $state(40);
	let padding = $state(32);

	// Layout
	let layoutMode = $state<LayoutMode>('auto');

	// Zoom regions
	let zoomRegions = $state<ZoomRegion[]>([]);

	// Cursor settings
	let cursorSettings = $state<CursorSettings>({
		enabled: true,
		size: 3,
		smoothing: 50,
		highlightClicks: true,
		highlightColor: '#3b82f6',
		highlightOpacity: 40,
		hideWhenIdle: false,
		idleTimeout: 3,
	});

	// Export
	let exportFormat = $state<ExportFormat>('mp4');
	let exportProgress = $state<number | null>(null);
	let isExporting = $state(false);

	// Undo/Redo stacks (simplified — stores snapshots of key settings)
	let undoStack = $state<string[]>([]);
	let redoStack = $state<string[]>([]);

	// Timeline zoom
	let timelineZoom = $state(1); // 1x = fit to width

	function getSettingsSnapshot(): string {
		return JSON.stringify({
			backgroundType,
			backgroundValue,
			backgroundBlur,
			padding,
			trimStart,
			trimEnd,
			zoomRegions,
			cursorSettings,
			layoutMode,
		});
	}

	function pushUndoState() {
		undoStack = [...undoStack, getSettingsSnapshot()];
		redoStack = [];
	}

	function undo() {
		if (undoStack.length === 0) return;
		const prev = undoStack[undoStack.length - 1];
		redoStack = [...redoStack, getSettingsSnapshot()];
		undoStack = undoStack.slice(0, -1);
		applySnapshot(prev);
	}

	function redo() {
		if (redoStack.length === 0) return;
		const next = redoStack[redoStack.length - 1];
		undoStack = [...undoStack, getSettingsSnapshot()];
		redoStack = redoStack.slice(0, -1);
		applySnapshot(next);
	}

	function applySnapshot(json: string) {
		const s = JSON.parse(json);
		backgroundType = s.backgroundType;
		backgroundValue = s.backgroundValue;
		backgroundBlur = s.backgroundBlur;
		padding = s.padding;
		trimStart = s.trimStart;
		trimEnd = s.trimEnd;
		zoomRegions = s.zoomRegions;
		cursorSettings = s.cursorSettings;
		layoutMode = s.layoutMode;
	}

	function addZoomRegion(start: number, end: number, scale = 1.5) {
		pushUndoState();
		zoomRegions = [...zoomRegions, { id: generateId(), start, end, scale }];
	}

	function removeZoomRegion(id: string) {
		pushUndoState();
		zoomRegions = zoomRegions.filter((z) => z.id !== id);
	}

	function updateZoomRegion(id: string, updates: Partial<ZoomRegion>) {
		zoomRegions = zoomRegions.map((z) => (z.id === id ? { ...z, ...updates } : z));
	}

	function reset() {
		currentTime = 0;
		isPlaying = false;
		trimStart = 0;
		trimEnd = metadata?.duration ?? 0;
		backgroundType = 'wallpaper';
		backgroundValue = WALLPAPERS[0].src;
		backgroundBlur = 40;
		padding = 32;
		layoutMode = 'auto';
		zoomRegions = [];
		cursorSettings = {
			enabled: true,
			size: 3,
			smoothing: 50,
			highlightClicks: true,
			highlightColor: '#3b82f6',
			highlightOpacity: 40,
			hideWhenIdle: false,
			idleTimeout: 3,
		};
		undoStack = [];
		redoStack = [];
	}

	return {
		// Getters (reactive reads)
		get videoPath() { return videoPath; },
		set videoPath(v: string) { videoPath = v; },

		get metadata() { return metadata; },
		set metadata(v: VideoMetadata | null) { metadata = v; },

		get thumbnailStrip() { return thumbnailStrip; },
		set thumbnailStrip(v: string[]) { thumbnailStrip = v; },

		get currentTime() { return currentTime; },
		set currentTime(v: number) { currentTime = v; },

		get isPlaying() { return isPlaying; },
		set isPlaying(v: boolean) { isPlaying = v; },

		get trimStart() { return trimStart; },
		set trimStart(v: number) { pushUndoState(); trimStart = v; },

		get trimEnd() { return trimEnd; },
		set trimEnd(v: number) { pushUndoState(); trimEnd = v; },

		get backgroundType() { return backgroundType; },
		set backgroundType(v: BackgroundType) { pushUndoState(); backgroundType = v; },

		get backgroundValue() { return backgroundValue; },
		set backgroundValue(v: string) { pushUndoState(); backgroundValue = v; },

		get backgroundBlur() { return backgroundBlur; },
		set backgroundBlur(v: number) { backgroundBlur = v; },

		get padding() { return padding; },
		set padding(v: number) { padding = v; },

		get layoutMode() { return layoutMode; },
		set layoutMode(v: LayoutMode) { pushUndoState(); layoutMode = v; },

		get zoomRegions() { return zoomRegions; },

		get cursorSettings() { return cursorSettings; },
		set cursorSettings(v: CursorSettings) { cursorSettings = v; },

		get exportFormat() { return exportFormat; },
		set exportFormat(v: ExportFormat) { exportFormat = v; },

		get exportProgress() { return exportProgress; },
		set exportProgress(v: number | null) { exportProgress = v; },

		get isExporting() { return isExporting; },
		set isExporting(v: boolean) { isExporting = v; },

		get timelineZoom() { return timelineZoom; },
		set timelineZoom(v: number) { timelineZoom = v; },

		get canUndo() { return undoStack.length > 0; },
		get canRedo() { return redoStack.length > 0; },

		// Methods
		undo,
		redo,
		pushUndoState,
		addZoomRegion,
		removeZoomRegion,
		updateZoomRegion,
		reset,
	};
}

export type EditorStore = ReturnType<typeof createEditorStore>;
