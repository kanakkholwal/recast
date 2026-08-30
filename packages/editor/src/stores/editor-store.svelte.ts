/**
 * Editor Store: Central reactive state for the video editor.
 * Uses Svelte 5 runes ($state) for granular reactivity.
 *
 * The document model it reads and writes lives in `$lib/editor/render-state`,
 * re-exported below so existing `editor-store` importers keep their specifier.
 */

import { type CaptionStyle, DEFAULT_CAPTION_STYLE } from "@recast/captions";
import { backgroundNeedsShadow, migrateBackgroundValue } from "@recast/design/backgrounds";
import {
	clampPlacement,
	keyframesFromMotionSegments,
} from "../components/_components/camera-overlay.logic";
import { resolveTokenRgb, resolveTokenRgba } from "../lib/annotations/canvas-tokens";
import {
	type AudioClip,
	type AudioClipSource,
	defaultAudioClip,
	splitClip,
	voiceClip,
} from "../lib/audio/music";
import { scaleTranscript, transcriptTimeScale } from "../lib/captions/normalize";
import type { CursorSampleLike } from "../lib/cursor/smoothing";
import { EASE, EASE_IN_OUT, type Easing } from "../lib/easing/cubic-bezier";
import {
	type Annotation,
	type AnnotationKind,
	type AnnotationKindName,
	type AudioSettings,
	type BackgroundSelection,
	type BackgroundType,
	type CameraOverlaySettings,
	type CameraPlacement,
	type CaptionExportOptions,
	type CursorSettings,
	cameraPlacementFromPreset,
	clampFramePaddingPercent,
	DEFAULT_ANNOTATION_FILL,
	DEFAULT_ANNOTATION_RAMP,
	DEFAULT_ANNOTATION_STROKE,
	DEFAULT_CAPTION_EXPORT,
	DEFAULT_GIF_SETTINGS,
	DEFAULT_ZOOM_CENTER,
	DEFAULT_ZOOM_MOTION_BLUR,
	DEFAULT_ZOOM_RAMP,
	type DeleteSelectionResult,
	type EditorRenderState,
	type EditorSelection,
	type ExportFormat,
	type ExportQuality,
	type ExportSpeed,
	type GifSettings,
	generateId,
	type LayoutMode,
	normalizeFramePaddingPercent,
	type OutputAspect,
	type PanelTab,
	type ShadowSettings,
	type TimelineCommands,
	type TimelineTool,
	type Transcript,
	type VideoMetadata,
	WALLPAPERS,
	wallpaperBackgroundValue,
	type ZoomRegion,
} from "../lib/editor/render-state";
import type { TimeMode } from "../lib/editor/time";
import { log } from "../lib/log";
import { resolveBackgroundWireValue } from "../lib/registry/resolve";
import {
	setSeamTransition as applySeamTransition,
	seamTransitionAt as readSeamTransition,
	type SeamTransition,
} from "../lib/scenes/seam";
import {
	segmentAnimAt as animAtAnchor,
	type MotionTone,
	pruneSegmentAnims,
	retuneAnimsForTone,
	type SceneAnimSpec,
	type SegmentAnim,
	setSegmentAnim as upsertSegmentAnim,
} from "../lib/scenes/segment-anim";
import { type CutSource, type TimelineCut, totalCutDuration } from "../lib/timeline/cuts";
import {
	buildSpeedOf,
	pruneSegmentSpeeds,
	type SegmentSpeed,
	segmentSpeedAt as speedAtAnchor,
	segmentSpeedAtTime as speedAtTime,
	setSegmentSpeed as upsertSegmentSpeed,
} from "../lib/timeline/segment-speed";
import {
	deriveSegments,
	planDeleteSegment,
	planSplit,
	type Segment,
	segmentAt,
} from "../lib/timeline/segments";
import {
	buildGapMap,
	displayTimeMap,
	originalToOutput,
	outputToOriginal,
	timeMapFromSegments,
} from "../lib/timeline/time-map";
import { experimentalStore } from "./experimental.svelte";

export * from "../lib/editor/render-state";

export function createEditorStore() {
	// Video source
	let videoPath = $state("");
	let cursorPath = $state<string | null>(null);
	// Raw on-disk media paths, needed by Rust analysis commands such as silence detection.
	let recordingPath = $state<string | null>(null);
	let audioPath = $state<string | null>(null);
	let microphonePath = $state<string | null>(null);
	// Generated transcript (session-only; project persistence deferred) plus how it renders.
	let transcript = $state.raw<Transcript | null>(null);
	let captionStyle = $state<CaptionStyle>({ ...DEFAULT_CAPTION_STYLE });
	let metadata = $state<VideoMetadata | null>(null);
	// The transcript is timed on the AUDIO axis while the playhead is VIDEO SOURCE time; count-based CFR makes them differ.
	let captionAudioDurationSec = $state<number | null>(null);
	// Memoized so the per-frame caption redraw doesn't re-map every word.
	const captionTranscriptMemo = $derived(
		scaleTranscript(transcript, transcriptTimeScale(metadata?.duration, captionAudioDurationSec)),
	);
	// `$state.raw`: replaced wholesale, never mutated element-wise, so deep proxying is pure overhead.
	let thumbnailStrip = $state.raw<string[]>([]);
	// Audio peak envelope (0..1 per bucket) for the timeline waveform. Transient.
	let waveform = $state.raw<number[]>([]);

	// Playback
	let currentTime = $state(0);
	let isPlaying = $state(false);
	// `seek()` moves the playhead AND the transport together; setting `currentTime` alone loses to the next time publish.
	let seekHandler: ((time: number) => void) | null = null;
	// Registered by Timeline.svelte on mount (see TimelineCommands).
	let timelineCommands: TimelineCommands | null = null;

	// Reset to 'select' on every document load: a mode must not survive into a different recording.
	let timelineTool = $state<TimelineTool>("select");

	// Trim
	let trimStart = $state(0);
	let trimEnd = $state(0); // will be set to duration on load

	// Silence / manual cuts: removed ranges, in original-recording seconds.
	let cuts = $state<TimelineCut[]>([]);
	// Editing aid only: no export effect until a segment between two boundaries is ripple-deleted.
	let splitPoints = $state<number[]>([]);
	let segmentSpeeds = $state<SegmentSpeed[]>([]);
	// Anchored to a segment's original start; a segment with no entry is static.
	let segmentAnims = $state<SegmentAnim[]>([]);
	// Authoring-only: it bakes concrete values into each spec, so the export never reads it.
	let motionTone = $state<MotionTone>("balanced");
	// Transient: flips the timeline onto the full-recording axis so a trim handle can reveal the head and tail.
	let isTrimming = $state(false);
	// Transient UI selection: start time of the highlighted clip block (mirrors selectedZoomRegionId).
	let selectedClipStart = $state<number | null>(null);
	// Transient UI selection: the highlighted cut band's id, or null.
	let selectedCutId = $state<string | null>(null);
	// Transient UI selection: the highlighted music/audio clip's id, or null.
	let selectedMusicClipId = $state<string | null>(null);
	// Rendering only: cuts show as restorable gaps while playback and export stay continuous.
	let showCutGaps = $state(false);
	// Persisted so a re-scan or reopen doesn't resurface ranges the user already rejected.
	let dismissedSilences = $state<Array<{ start: number; end: number }>>([]);
	// Bypasses the lane in preview and export while keeping its data, so the toggle is reversible.
	let cutsEnabled = $state(true);
	let focusEnabled = $state(true);

	// Background
	let backgroundType = $state<BackgroundType>("wallpaper");
	let backgroundValue = $state(wallpaperBackgroundValue(WALLPAPERS[0].id));
	let backgroundBlur = $state(40);
	let padding = $state(3);
	let borderRadius = $state(0); // 0..50 (% of shorter video edge)

	// Drop shadow cast by the video rect onto the background.
	let shadow = $state<ShadowSettings>({
		enabled: false,
		blur: 40,
		spread: 0,
		offsetY: 24,
		opacity: 40,
		color: "#000000",
	});

	// Layout
	let layoutMode = $state<LayoutMode>("auto");

	// 'source' follows the input video; any other value reframes the canvas with letterbox or pillarbox bars.
	let outputAspect = $state<OutputAspect>("source");

	// UI affordance only, so the toolbar can show which preset is in effect; cleared on reset to source.
	let lastAppliedPresetId = $state<string | null>(null);

	// `$state.raw`: tens of thousands of samples, replaced on load. Set by VideoPreview, read-only elsewhere.
	let cursorSamplesRaw = $state.raw<CursorSampleLike[]>([]);
	// Idle spans (us) for the idle-hide fade; the browser export reads them for parity.
	let cursorIdlePeriods = $state.raw<{ startUs: number; endUs: number }[]>([]);

	// Annotations + active tool (for the preview canvas's place-mode).
	let annotations = $state<Annotation[]>([]);
	let selectedAnnotationId = $state<string | null>(null);
	let annotationTool = $state<AnnotationKindName | null>(null);
	// Layer-panel hover: the overlay flashes the matching annotation so a layer is findable in a busy frame.
	let hoveredAnnotationId = $state<string | null>(null);
	// Independent of per-annotation `hidden`, so the master toggle never tramples user state.
	let annotationsGloballyHidden = $state<boolean>(false);
	// Snap engine on/off. Default on. Alt held during drag bypasses regardless.
	let annotationSnapEnabled = $state<boolean>(true);
	// Monotonic so new annotations start above existing ones and ordering survives reorders.
	let annotationZSeq = 1;

	// Zoom regions
	let zoomRegions = $state<ZoomRegion[]>([]);
	let selectedZoomRegionId = $state<string | null>(null);
	// `enabled` is the user preference; `applied` is the latch that stops a re-run on every reopen.
	let autoZoomEnabled = $state(true);
	let autoZoomApplied = $state(false);

	// Overlays gate their editing UI on this, so handles never appear for a hidden panel's feature.
	let activePanel = $state<PanelTab>("canvas");

	// Lives here so the transport readout and the timeline agree; timeline-local, the player had its own format.
	let timeMode = $state<TimeMode>("smpte");

	// `null` means linear; a curve reshapes the per-sample lerp in the WebGL preview.
	let cursorMotionEasing = $state<Easing | null>(null);

	// Cursor settings
	let cursorSettings = $state<CursorSettings>({
		enabled: true,
		size: 2,
		style: "dot",
		smoothing: 50,
		snapToClicks: true,
		snapWindowMs: 80,
		highlightClicks: true,
		highlightColor: "#3b82f6",
		highlightOpacity: 40,
		hideWhenIdle: false,
		idleTimeout: 3,
		motionBlur: 0,
		clickBounce: 0,
		bounceSpeedMs: 220,
		sway: 0,
	});

	// Audio settings
	let audioSettings = $state<AudioSettings>({
		volume: 100,
		muted: false,
		systemVolume: 100,
		systemMuted: false,
		micVolume: 100,
		micMuted: false,
		fadeIn: 0,
		fadeOut: 0,
		normalizeLoudness: false,
	});

	// Music / extra-audio clips laid on the output timeline (mixed in at export).
	let musicClips = $state<AudioClip[]>([]);

	// Phase 1 defaults: bottom-right, 16% size, 1:1, 16% radius, mirrored, in normalized UV so aspect changes survive.
	let cameraOverlay = $state<CameraOverlaySettings>({
		enabled: false,
		mirror: true,
		shape: "rounded",
		cornerRadius: 0.16,
		animationPreset: "soft",
		zoomFollow: true,
		zoomFollowStrength: 0.6,
		zoomFollowDuration: 0.4,
		zoomFollowEasing: { ...EASE_IN_OUT },
		defaultPlacement: cameraPlacementFromPreset("bottom-right"),
		motionSegments: [],
		keyframes: [],
		keyframeEasing: { ...EASE_IN_OUT },
		shadow: 0.35,
	});

	// Export
	let exportFormat = $state<ExportFormat>("mp4");
	// Source resolution by default: downscaling a 1080p+ screen recording softens sharp text and UI.
	let exportQuality = $state<ExportQuality>("source");
	let exportSpeed = $state<ExportSpeed>("balanced");
	// `null` keeps the source rate; a number only ever downsamples. GIF has its own fps in `gifSettings`.
	let exportFps = $state<number | null>(null);
	// A >60fps recording exports far faster at 60; the flag stops a later metadata set clobbering the user's choice.
	let exportFpsDefaulted = false;
	let gifSettings = $state<GifSettings>({ ...DEFAULT_GIF_SETTINGS });
	// Burn-in or sidecar; session-only, like the other export prefs.
	let captionExport = $state<CaptionExportOptions>({ ...DEFAULT_CAPTION_EXPORT });
	let exportProgress = $state<number | null>(null);
	let isExporting = $state(false);

	// `$state.raw`: only array identity is read reactively and snapshots are already plain; bounded to MAX_UNDO_HISTORY.
	type EditorSettings = ReturnType<typeof captureSettings>;
	let undoStack = $state.raw<EditorSettings[]>([]);
	let redoStack = $state.raw<EditorSettings[]>([]);

	// True from the first undoable edit; cleared by markSaved or by loading a fresh render state.
	let isDirty = $state(false);
	let lastSavedAt = $state<number | null>(null);
	// Frozen last-on-disk state, so `revertToSaved` drops every unsaved edit without walking the undo stack.
	let savedSnapshot = $state<EditorSettings | null>(null);

	// Timeline zoom
	let timelineZoom = $state(1); // 1x = fit to width

	// Anything omitted here survives `pushUndoState` but is not restored on undo. Keep in sync with `applySnapshot`.
	function captureSettings() {
		return {
			backgroundType,
			backgroundValue,
			backgroundBlur,
			padding,
			borderRadius,
			shadow,
			trimStart,
			trimEnd,
			zoomRegions,
			cuts,
			splitPoints,
			segmentSpeeds,
			segmentAnims,
			motionTone,
			autoZoomEnabled,
			autoZoomApplied,
			annotations,
			cursorSettings,
			audioSettings,
			cameraOverlay,
			layoutMode,
			outputAspect,
			lastAppliedPresetId,
			cursorMotionEasing,
			musicClips,
		};
	}

	// `$state.snapshot`, not a JSON round-trip, so undo/redo never pays stringify+parse on the press.
	function getSettingsSnapshot(): EditorSettings {
		return $state.snapshot(captureSettings()) as EditorSettings;
	}

	const MAX_UNDO_HISTORY = 50;

	let undoSuppression = 0;

	function pushUndoState() {
		if (undoSuppression > 0) return;
		undoStack = [...undoStack, getSettingsSnapshot()].slice(-MAX_UNDO_HISTORY);
		redoStack = [];
		isDirty = true;
	}

	/**
	 * Run `fn` without recording undo history or marking the project dirty —
	 * for transient writes the user hasn't committed, like previewing a preset
	 * as the cursor moves over it. Commit the real change outside this scope.
	 */
	function withoutUndo(fn: () => void) {
		const wasDirty = isDirty;
		undoSuppression++;
		try {
			fn();
		} finally {
			undoSuppression--;
			isDirty = wasDirty;
		}
	}

	// Unwinds a push that turned into a no-op, e.g. a placement cancelled before it changed anything.
	function popUndoState() {
		if (undoStack.length > 0) undoStack = undoStack.slice(0, -1);
	}

	// Edits sharing a `key` within `ttlMs` collapse into one entry, so a 30-frame nudge is one Ctrl+Z.
	let lastCoalesceKey: string | null = null;
	let lastCoalesceAt = 0;
	function pushUndoStateCoalesced(key: string, ttlMs = 500) {
		const now = typeof performance !== "undefined" ? performance.now() : Date.now();
		if (lastCoalesceKey === key && now - lastCoalesceAt < ttlMs) {
			lastCoalesceAt = now;
			isDirty = true;
			return;
		}
		lastCoalesceKey = key;
		lastCoalesceAt = now;
		pushUndoState();
	}

	function markSaved(savedAtUnixMs: number) {
		isDirty = false;
		lastSavedAt = savedAtUnixMs;
		savedSnapshot = getSettingsSnapshot();
	}

	function revertToSaved() {
		if (!savedSnapshot) return;
		// Push first so the revert itself is undoable and a mistaken revert can be walked back.
		undoStack = [...undoStack, getSettingsSnapshot()].slice(-MAX_UNDO_HISTORY);
		redoStack = [];
		applySnapshot(savedSnapshot);
		isDirty = false;
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

	// Fields are copied, never aliased: the snapshot stays immutable across re-applies.
	function applySnapshot(s: EditorSettings) {
		backgroundType = s.backgroundType;
		backgroundValue = s.backgroundValue;
		backgroundBlur = s.backgroundBlur;
		padding = normalizeFramePaddingPercent(s.padding, metadata);
		borderRadius = s.borderRadius ?? 0;
		shadow = s.shadow ? { ...s.shadow } : shadow;
		musicClips = (s.musicClips ?? []).map((c) => ({ ...c }));
		if (selectedMusicClipId && !musicClips.find((c) => c.id === selectedMusicClipId)) {
			selectedMusicClipId = null;
		}
		trimStart = s.trimStart;
		trimEnd = s.trimEnd;
		zoomRegions = (s.zoomRegions ?? []).map((r: ZoomRegion) => ({
			...r,
			centerX: r.centerX ?? DEFAULT_ZOOM_CENTER,
			centerY: r.centerY ?? DEFAULT_ZOOM_CENTER,
			motionBlur: r.motionBlur ?? DEFAULT_ZOOM_MOTION_BLUR,
			source: r.source ?? "manual",
		}));
		autoZoomEnabled = s.autoZoomEnabled ?? autoZoomEnabled;
		autoZoomApplied = s.autoZoomApplied ?? autoZoomApplied;
		cuts = (s.cuts ?? []).map((c: TimelineCut) => ({ ...c }));
		splitPoints = [...(s.splitPoints ?? [])];
		segmentSpeeds = (s.segmentSpeeds ?? []).map((o: SegmentSpeed) => ({ ...o }));
		segmentAnims = (s.segmentAnims ?? []).map((o: SegmentAnim) => ({ ...o }));
		motionTone = s.motionTone ?? "balanced";
		// Entries keep their snapshot ids, so refs like `selectedAnnotationId` survive the undo.
		if (Array.isArray(s.annotations)) {
			annotations = s.annotations.map((a: Annotation) => ({ ...a }));
			annotationZSeq = annotations.length + 1;
			if (selectedAnnotationId && !annotations.find((a) => a.id === selectedAnnotationId)) {
				selectedAnnotationId = null;
			}
			if (hoveredAnnotationId && !annotations.find((a) => a.id === hoveredAnnotationId)) {
				hoveredAnnotationId = null;
			}
		}
		cursorSettings = { ...s.cursorSettings };
		// Old projects have no per-track volumes; fall back to master so their mix stays 100% system and mic.
		if (s.audioSettings) {
			const loaded = s.audioSettings;
			audioSettings = {
				volume: loaded.volume,
				muted: loaded.muted,
				systemVolume: loaded.systemVolume ?? loaded.volume,
				systemMuted: loaded.systemMuted ?? loaded.muted,
				micVolume: loaded.micVolume ?? loaded.volume,
				micMuted: loaded.micMuted ?? loaded.muted,
				fadeIn: loaded.fadeIn,
				fadeOut: loaded.fadeOut,
				normalizeLoudness: loaded.normalizeLoudness ?? false,
			};
		} else {
			audioSettings = audioSettings;
		}
		// Camera overlay was captured but never restored, which silently destroyed overlay edits on undo.
		if (s.cameraOverlay) {
			cameraOverlay = {
				...s.cameraOverlay,
				defaultPlacement: { ...s.cameraOverlay.defaultPlacement },
				motionSegments: (s.cameraOverlay.motionSegments ?? []).map(
					(seg: CameraOverlaySettings["motionSegments"][number]) => ({ ...seg }),
				),
				keyframes: (s.cameraOverlay.keyframes ?? []).map((k) => ({
					atSec: k.atSec,
					placement: { ...k.placement },
				})),
				keyframeEasing: { ...(s.cameraOverlay.keyframeEasing ?? EASE_IN_OUT) },
			};
		}
		layoutMode = s.layoutMode;
		outputAspect = s.outputAspect ?? "source";
		lastAppliedPresetId = s.lastAppliedPresetId ?? null;
		cursorMotionEasing = s.cursorMotionEasing ?? null;
	}

	function addZoomRegion(
		start: number,
		end: number,
		scale = 1.5,
		center?: { x: number; y: number },
	) {
		pushUndoState();
		const region: ZoomRegion = {
			id: generateId(),
			start,
			end,
			scale,
			easeIn: { ...EASE },
			easeOut: { ...EASE },
			rampIn: DEFAULT_ZOOM_RAMP,
			rampOut: DEFAULT_ZOOM_RAMP,
			centerX: center?.x ?? DEFAULT_ZOOM_CENTER,
			centerY: center?.y ?? DEFAULT_ZOOM_CENTER,
			motionBlur: DEFAULT_ZOOM_MOTION_BLUR,
			source: "manual",
		};
		zoomRegions = [...zoomRegions, region];
		selectZoomRegion(region.id);
		log.info("focus", "zoom_added", { id: region.id, start, end, scale });
		return region.id;
	}

	/**
	 * Append an auto-generated zoom region without pushing undo (the caller
	 * batches all auto-applied regions into a single undo entry). Returns
	 * the new id so callers can correlate with their suggestion.
	 */
	function addAutoZoomRegion(
		start: number,
		end: number,
		scale: number,
		centerX: number,
		centerY: number,
	) {
		const region: ZoomRegion = {
			id: generateId(),
			start,
			end,
			scale,
			easeIn: { ...EASE },
			easeOut: { ...EASE },
			rampIn: DEFAULT_ZOOM_RAMP,
			rampOut: DEFAULT_ZOOM_RAMP,
			centerX,
			centerY,
			motionBlur: DEFAULT_ZOOM_MOTION_BLUR,
			source: "auto",
		};
		zoomRegions = [...zoomRegions, region];
		return region.id;
	}

	function clearAutoZooms() {
		const hasAuto = zoomRegions.some((z) => z.source === "auto");
		if (!hasAuto) return;
		pushUndoState();
		zoomRegions = zoomRegions.filter((z) => z.source !== "auto");
		if (selectedZoomRegionId && !zoomRegions.find((z) => z.id === selectedZoomRegionId)) {
			selectedZoomRegionId = null;
		}
	}

	function setBackground(selection: BackgroundSelection) {
		const hasChanged = backgroundType !== selection.type || backgroundValue !== selection.value;
		if (!hasChanged) return;
		pushUndoState();
		backgroundType = selection.type;
		backgroundValue = selection.value;
		// A backdrop close in luminance to the recording leaves no visible edge; only ever turned on, never off.
		if (!shadow.enabled && backgroundNeedsShadow(selection.value)) {
			shadow = { ...shadow, enabled: true };
			log.info("background", "auto-enabled drop shadow for low-separation backdrop");
		}
		// `value` can be a long wallpaper/gradient string, so log only the type.
		log.info("background", "changed", { type: selection.type });
	}

	/**
	 * Stream a background value during a continuous gesture (dragging a
	 * gradient stop's color/position or the angle). Updates fire live so the
	 * WebGL preview tracks the drag, but the whole gesture coalesces into a
	 * single undo entry (mirrors the keyboard-nudge / slider pattern) instead
	 * of spamming one push per pointer-move. Discrete actions (presets, add /
	 * remove stop) should use {@link setBackground} for a clean undo step.
	 */
	function setBackgroundLive(type: BackgroundType, value: string) {
		pushUndoStateCoalesced("background-live");
		backgroundType = type;
		backgroundValue = value;
		isDirty = true;
	}

	/**
	 * Stream the cursor easing curve during a bezier-handle drag. The plain
	 * `cursorMotionEasing` setter pushes an undo entry per assignment, which the
	 * drag fires on every pointermove — one gesture buried the stack in dozens of
	 * entries. Discrete picks (presets) keep using the setter.
	 */
	function setCursorMotionEasingLive(v: Easing | null) {
		pushUndoStateCoalesced("cursor-easing-live");
		cursorMotionEasing = v;
		isDirty = true;
	}

	function updateCursorSettings(updates: Partial<CursorSettings>) {
		cursorSettings = { ...cursorSettings, ...updates };
		// Sliders (size, smoothing) fire continuously, so debounce to one line.
		log.debounced("cursor-settings", "cursor", "settings_changed", { ...updates });
	}

	function updateAudioSettings(updates: Partial<AudioSettings>) {
		audioSettings = { ...audioSettings, ...updates };
		log.debounced("audio-settings", "audio", "settings_changed", { ...updates });
	}

	function updateShadow(updates: Partial<ShadowSettings>) {
		shadow = { ...shadow, ...updates };
	}

	function addMusicClip(source: AudioClipSource): AudioClip {
		pushUndoState();
		const clip = defaultAudioClip(generateId(), source);
		musicClips = [...musicClips, clip];
		return clip;
	}
	function updateMusicClip(id: string, updates: Partial<AudioClip>) {
		musicClips = musicClips.map((c) => (c.id === id ? { ...c, ...updates } : c));
	}
	function removeMusicClip(id: string, pushUndo = true) {
		if (pushUndo) pushUndoState();
		musicClips = musicClips.filter((c) => c.id !== id);
		if (selectedMusicClipId === id) selectedMusicClipId = null;
	}

	/** Split a music clip at an OUTPUT-axis time into two. Returns true if it split
	 *  (the point must be strictly inside the clip). The left half keeps the id. */
	function splitMusicClip(id: string, atOutputSec: number): boolean {
		const clip = musicClips.find((c) => c.id === id);
		if (!clip) return false;
		const parts = splitClip(clip, atOutputSec, timeMapMemo.outputDuration, generateId());
		if (!parts) return false;
		pushUndoState();
		musicClips = musicClips.flatMap((c) => (c.id === id ? parts : [c]));
		return true;
	}

	// 'Detached' is just 'a voice clip exists', so there is no separate flag to keep in sync.
	const audioDetached = $derived(musicClips.some((c) => c.role === "voice"));
	const canDetachAudio = $derived(!!(audioPath || microphonePath));
	const voiceClips = $derived(musicClips.filter((c) => c.role === "voice"));
	const musicOnlyClips = $derived(musicClips.filter((c) => c.role !== "voice"));

	/** Split system+mic into independent `voice` clips (their per-source gain/mute
	 *  and the timeline fades carried over) and suppress the monolithic source path.
	 *  No-op if already detached or the recording has no separate audio to detach. */
	function detachRecordingAudio(): boolean {
		if (audioDetached || !canDetachAudio) return false;
		const common = {
			offsetSec: Math.max(0, trimStart),
			fadeIn: audioSettings.fadeIn,
			fadeOut: audioSettings.fadeOut,
		};
		const made: AudioClip[] = [];
		if (audioPath) {
			made.push(
				voiceClip(generateId(), audioPath, {
					...common,
					gain: audioSettings.systemVolume,
					muted: audioSettings.systemMuted,
				}),
			);
		}
		if (microphonePath) {
			made.push(
				voiceClip(generateId(), microphonePath, {
					...common,
					gain: audioSettings.micVolume,
					muted: audioSettings.micMuted,
				}),
			);
		}
		if (made.length === 0) return false;
		pushUndoState();
		musicClips = [...musicClips, ...made];
		return true;
	}

	/** Re-bind the recording audio: drop every voice clip so the source path resumes. */
	function reattachRecordingAudio(): boolean {
		if (!audioDetached) return false;
		pushUndoState();
		musicClips = musicClips.filter((c) => c.role !== "voice");
		if (selectedMusicClipId && !musicClips.find((c) => c.id === selectedMusicClipId)) {
			selectedMusicClipId = null;
		}
		return true;
	}

	/**
	 * Patch the camera overlay settings. Mirrors `updateCursorSettings`
	 * shape; callers handle their own `pushUndoState` so coalesced
	 * interactions (drag, slider) can batch into a single undo entry.
	 */
	function updateCameraOverlay(updates: Partial<CameraOverlaySettings>) {
		cameraOverlay = { ...cameraOverlay, ...updates };
	}

	/**
	 * Stream a camera-overlay edit during a continuous gesture (a bezier-handle
	 * drag fires on every pointermove). `coalesceKey` scopes the window so two
	 * different curves in the same panel don't merge into one undo entry.
	 */
	function updateCameraOverlayLive(updates: Partial<CameraOverlaySettings>, coalesceKey: string) {
		pushUndoStateCoalesced(coalesceKey);
		cameraOverlay = { ...cameraOverlay, ...updates };
		isDirty = true;
	}

	// Per-cut mode upserts a keyframe at the playhead; otherwise it sets the single static placement.
	function setCameraPlacement(placement: CameraPlacement) {
		if (cameraOverlay.keyframes.length > 0) {
			const atSec = currentTime;
			const next = cameraOverlay.keyframes.filter((k) => Math.abs(k.atSec - atSec) > 0.05);
			next.push({ atSec, placement });
			next.sort((a, b) => a.atSec - b.atSec);
			cameraOverlay = { ...cameraOverlay, keyframes: next };
		} else {
			cameraOverlay = { ...cameraOverlay, defaultPlacement: placement };
		}
	}

	// On seeds a keyframe at the playhead from the static placement so nothing jumps; off drops the keyframes.
	function setCameraPerCut(on: boolean) {
		pushUndoState();
		if (on && cameraOverlay.keyframes.length === 0) {
			cameraOverlay = {
				...cameraOverlay,
				keyframes: [{ atSec: currentTime, placement: { ...cameraOverlay.defaultPlacement } }],
			};
		} else if (!on) {
			cameraOverlay = { ...cameraOverlay, keyframes: [] };
		}
	}

	/** Remove the keyframe nearest the playhead (within 0.15s), for the panel's
	 *  "clear this cut's position" affordance. */
	function removeCameraKeyframeNear(atSec: number) {
		const next = cameraOverlay.keyframes.filter((k) => Math.abs(k.atSec - atSec) > 0.15);
		if (next.length === cameraOverlay.keyframes.length) return;
		pushUndoState();
		cameraOverlay = { ...cameraOverlay, keyframes: next };
	}

	// --- Selection: exactly one thing at a time. Selecting clears the others, and Delete is a document-level command over `selection`.

	function selectClip(start: number | null) {
		selectedClipStart = start;
		if (start === null) return;
		selectedZoomRegionId = null;
		selectedAnnotationId = null;
		selectedCutId = null;
		selectedMusicClipId = null;
	}

	function selectZoomRegion(id: string | null) {
		selectedZoomRegionId = id;
		if (id === null) return;
		selectedClipStart = null;
		selectedAnnotationId = null;
		selectedCutId = null;
		selectedMusicClipId = null;
	}

	function selectAnnotation(id: string | null) {
		selectedAnnotationId = id;
		if (id === null) return;
		selectedClipStart = null;
		selectedZoomRegionId = null;
		selectedCutId = null;
		selectedMusicClipId = null;
	}

	function selectCut(id: string | null) {
		selectedCutId = id;
		if (id === null) return;
		selectedClipStart = null;
		selectedZoomRegionId = null;
		selectedAnnotationId = null;
		selectedMusicClipId = null;
	}

	function selectMusicClip(id: string | null) {
		selectedMusicClipId = id;
		if (id === null) return;
		selectedClipStart = null;
		selectedZoomRegionId = null;
		selectedAnnotationId = null;
		selectedCutId = null;
	}

	function clearSelection() {
		selectedClipStart = null;
		selectedZoomRegionId = null;
		selectedAnnotationId = null;
		selectedCutId = null;
		selectedMusicClipId = null;
	}

	const selection = $derived.by<EditorSelection | null>(() => {
		if (selectedAnnotationId !== null) {
			return { kind: "annotation", id: selectedAnnotationId };
		}
		if (selectedZoomRegionId !== null) {
			return { kind: "zoom", id: selectedZoomRegionId };
		}
		if (selectedCutId !== null) {
			return { kind: "cut", id: selectedCutId };
		}
		if (selectedMusicClipId !== null) {
			return { kind: "music", id: selectedMusicClipId };
		}
		if (selectedClipStart !== null) {
			return { kind: "clip", id: selectedClipStart };
		}
		return null;
	});

	/**
	 * Delete whatever is selected. Returns the playhead's new home for a clip
	 * delete (the join between the surviving segments), or null when nothing was
	 * deleted -- including a clip delete refused because it is the only segment.
	 */
	function deleteSelection(): DeleteSelectionResult | null {
		if (selectedAnnotationId !== null) {
			removeAnnotation(selectedAnnotationId);
			return { kind: "annotation", joinAt: null };
		}
		if (selectedZoomRegionId !== null) {
			removeZoomRegion(selectedZoomRegionId);
			return { kind: "zoom", joinAt: null };
		}
		if (selectedCutId !== null) {
			// Removing a cut restores the section; park on its (original) start.
			const cut = cuts.find((c) => c.id === selectedCutId);
			removeCut(selectedCutId);
			return { kind: "cut", joinAt: cut ? cut.start : null };
		}
		if (selectedMusicClipId !== null) {
			removeMusicClip(selectedMusicClipId);
			return { kind: "music", joinAt: null };
		}
		if (selectedClipStart !== null) {
			const joinAt = deleteSegmentAt(selectedClipStart);
			return joinAt === null ? null : { kind: "clip", joinAt };
		}
		return null;
	}

	function removeZoomRegion(id: string) {
		pushUndoState();
		zoomRegions = zoomRegions.filter((z) => z.id !== id);
		if (selectedZoomRegionId === id) selectedZoomRegionId = null;
		log.info("focus", "zoom_removed", { id });
	}

	/** Remove every zoom region in one undo step. */
	function clearZoomRegions() {
		if (zoomRegions.length === 0) return;
		pushUndoState();
		zoomRegions = [];
		selectedZoomRegionId = null;
		log.info("focus", "zoom_cleared_all", {});
	}

	/**
	 * Duplicate a region's settings into a new one placed immediately after it
	 * (back-to-back, same duration), clamped to the clip. A duplicate is always
	 * "manual" (it's an explicit user edit) and the copy becomes selected.
	 */
	function duplicateZoomRegion(id: string) {
		const src = zoomRegions.find((z) => z.id === id);
		if (!src) return;
		pushUndoState();
		const clipEnd = trimEnd || metadata?.duration || src.end;
		const duration = Math.max(0.1, src.end - src.start);
		let start = src.end;
		let end = start + duration;
		if (end > clipEnd) {
			// No room after the original: clamp the tail, and if the window collapses, stack it on top.
			end = clipEnd;
			start = Math.max(trimStart, end - duration);
			if (end - start < 0.1) {
				start = src.start;
				end = src.end;
			}
		}
		const copy: ZoomRegion = {
			...src,
			id: generateId(),
			easeIn: { ...src.easeIn },
			easeOut: { ...src.easeOut },
			start,
			end,
			source: "manual",
			hidden: src.hidden ?? false,
		};
		// Insert right after the source so list order matches the timeline.
		const idx = zoomRegions.findIndex((z) => z.id === id);
		zoomRegions = [...zoomRegions.slice(0, idx + 1), copy, ...zoomRegions.slice(idx + 1)];
		selectZoomRegion(copy.id);
		log.info("focus", "zoom_duplicated", { from: id, id: copy.id });
		return copy.id;
	}

	/** Toggle (or set) a region's hidden flag: non-destructive mute. */
	function setZoomRegionHidden(id: string, hidden?: boolean) {
		const src = zoomRegions.find((z) => z.id === id);
		if (!src) return;
		pushUndoState();
		const next = hidden ?? !(src.hidden ?? false);
		zoomRegions = zoomRegions.map((z) => (z.id === id ? { ...z, hidden: next } : z));
		log.info("focus", "zoom_hidden_toggled", { id, hidden: next });
	}

	function updateZoomRegion(id: string, updates: Partial<ZoomRegion>) {
		// Drag/resize/slider edits stream in, so debounce per region id.
		log.debounced(`zoom-${id}`, "focus", "zoom_updated", { id, ...updates });
		zoomRegions = zoomRegions.map((z) => {
			if (z.id !== id) return z;
			// A first user edit detaches an auto region, so 'Clear auto zooms' leaves tweaked ones alone.
			const next = { ...z, ...updates };
			if (z.source === "auto" && updates.source === undefined) {
				next.source = "manual";
			}
			return next;
		});
	}

	function addAnnotation(
		kind: AnnotationKind,
		start?: number,
		end?: number,
		overrides?: Partial<Pick<Annotation, "glow" | "name" | "anchor">>,
	): Annotation {
		pushUndoState();
		const clipEnd = trimEnd || metadata?.duration || 0;
		// Clamp into the trimmed clip so an annotation added at the trim end still yields a forward range.
		const now = Math.min(Math.max(currentTime, trimStart), clipEnd);
		let s = start ?? now;
		let e = end ?? Math.min(clipEnd, s + 2.0);
		if (!(e > s)) {
			s = Math.max(trimStart, clipEnd - 2.0);
			e = clipEnd;
		}
		// Theme colour rather than a fixed blue, resolved to a concrete value here because the export bakes it.
		const themeColor = resolveTokenRgb("var(--primary)");
		const annotation: Annotation = {
			id: generateId(),
			start: s,
			end: e,
			rampIn: DEFAULT_ANNOTATION_RAMP,
			rampOut: DEFAULT_ANNOTATION_RAMP,
			easeIn: { ...EASE },
			easeOut: { ...EASE },
			// Images start borderless (opt-in via Appearance); shapes keep the hairline stroke.
			stroke: {
				...DEFAULT_ANNOTATION_STROKE,
				color: themeColor,
				width: kind.kind === "image" ? 0 : DEFAULT_ANNOTATION_STROKE.width,
			},
			fill: resolveTokenRgba("var(--primary)", 0.18),
			kind,
			zIndex: annotationZSeq++,
			opacity: 1,
			...(overrides ?? {}),
		};
		annotations = [...annotations, annotation];
		selectAnnotation(annotation.id);
		log.info("annotation", "added", { id: annotation.id, kind: kind.kind });
		return annotation;
	}

	function updateAnnotation(id: string, updates: Partial<Annotation>) {
		// Position/style edits stream from drags + property sliders, so debounce.
		log.debounced(`annotation-${id}`, "annotation", "updated", {
			id,
			fields: Object.keys(updates),
		});
		annotations = annotations.map((a) => (a.id === id ? { ...a, ...updates } : a));
	}

	function removeAnnotation(id: string, pushUndo = true) {
		if (pushUndo) pushUndoState();
		annotations = annotations.filter((a) => a.id !== id);
		if (selectedAnnotationId === id) selectedAnnotationId = null;
		if (hoveredAnnotationId === id) hoveredAnnotationId = null;
		log.info("annotation", "removed", { id });
	}

	/** Sorted view by (zIndex, insertion-order). Higher z draws later.
	 *  Memoized: the overlay reads this twice per drawn frame, and as a plain
	 *  function every read allocated two arrays and re-sorted. */
	const annotationsByZOrdered = $derived.by(() =>
		[...annotations]
			.map((a, idx) => ({ a, idx, z: a.zIndex ?? idx }))
			.sort((a, b) => a.z - b.z || a.idx - b.idx)
			.map((e) => e.a),
	);

	function toggleAnnotationLock(id: string) {
		pushUndoState();
		annotations = annotations.map((a) =>
			a.id === id ? { ...a, locked: !(a.locked ?? false) } : a,
		);
	}

	function toggleAnnotationVisibility(id: string) {
		pushUndoState();
		annotations = annotations.map((a) =>
			a.id === id ? { ...a, hidden: !(a.hidden ?? false) } : a,
		);
	}

	function renameAnnotation(id: string, name: string) {
		const trimmed = name.trim();
		pushUndoState();
		annotations = annotations.map((a) => (a.id === id ? { ...a, name: trimmed || undefined } : a));
	}

	function duplicateAnnotation(id: string): Annotation | null {
		const source = annotations.find((a) => a.id === id);
		if (!source) return null;
		pushUndoState();
		const offset = 0.01;
		const dup: Annotation = JSON.parse(JSON.stringify(source));
		dup.id = generateId();
		dup.zIndex = annotationZSeq++;
		dup.name = source.name ? `${source.name} copy` : undefined;
		// Nudge the geometry diagonally so the duplicate is visible.
		if (
			dup.kind.kind === "rect" ||
			dup.kind.kind === "ellipse" ||
			dup.kind.kind === "image" ||
			dup.kind.kind === "text" ||
			dup.kind.kind === "blur"
		) {
			dup.kind = { ...dup.kind, x: dup.kind.x + offset, y: dup.kind.y + offset };
		} else if (dup.kind.kind === "arrow") {
			dup.kind = {
				...dup.kind,
				x1: dup.kind.x1 + offset,
				y1: dup.kind.y1 + offset,
				x2: dup.kind.x2 + offset,
				y2: dup.kind.y2 + offset,
			};
		}
		annotations = [...annotations, dup];
		selectAnnotation(dup.id);
		return dup;
	}

	/**
	 * Reorder by setting the annotation's `zIndex` relative to its neighbours.
	 * `direction = 1` brings forward, `-1` sends backward. Multiple steps will
	 * skip over multiple neighbours.
	 */
	function reorderAnnotation(id: string, direction: 1 | -1) {
		const ordered = annotationsByZOrdered;
		const idx = ordered.findIndex((a) => a.id === id);
		if (idx === -1) return;
		const targetIdx = idx + direction;
		if (targetIdx < 0 || targetIdx >= ordered.length) return;
		pushUndoState();
		// Strictly monotonic 1..N with the pair swapped, so repeated reorders stay stable.
		const next = [...ordered];
		[next[idx], next[targetIdx]] = [next[targetIdx], next[idx]];
		const zMap = new Map(next.map((a, i) => [a.id, i + 1]));
		annotations = annotations.map((a) => ({ ...a, zIndex: zMap.get(a.id) ?? a.zIndex }));
		annotationZSeq = next.length + 1;
	}

	/** Move to absolute z-position by id (used by drag-reorder in the layer panel). */
	function setAnnotationZOrder(orderedIds: string[]) {
		pushUndoState();
		const zMap = new Map(orderedIds.map((id, i) => [id, i + 1]));
		annotations = annotations.map((a) => (zMap.has(a.id) ? { ...a, zIndex: zMap.get(a.id)! } : a));
		annotationZSeq = orderedIds.length + 1;
	}

	function reset() {
		currentTime = 0;
		isPlaying = false;
		trimStart = 0;
		trimEnd = metadata?.duration ?? 0;
		backgroundType = "wallpaper";
		backgroundValue = wallpaperBackgroundValue(WALLPAPERS[0].id);
		backgroundBlur = 40;
		padding = 3;
		borderRadius = 0;
		shadow = {
			enabled: false,
			blur: 40,
			spread: 0,
			offsetY: 24,
			opacity: 40,
			color: "#000000",
		};
		musicClips = [];
		selectedMusicClipId = null;
		layoutMode = "auto";
		outputAspect = "source";
		lastAppliedPresetId = null;
		zoomRegions = [];
		selectedZoomRegionId = null;
		cuts = [];
		splitPoints = [];
		segmentSpeeds = [];
		segmentAnims = [];
		motionTone = "balanced";
		cutsEnabled = true;
		focusEnabled = true;
		dismissedSilences = [];
		autoZoomEnabled = true;
		autoZoomApplied = false;
		annotations = [];
		selectedAnnotationId = null;
		annotationTool = null;
		timelineTool = "select";
		hoveredAnnotationId = null;
		annotationsGloballyHidden = false;
		annotationSnapEnabled = true;
		annotationZSeq = 1;
		cursorMotionEasing = null;
		cursorSettings = {
			enabled: true,
			size: 2,
			style: "dot",
			smoothing: 50,
			snapToClicks: true,
			snapWindowMs: 80,
			highlightClicks: true,
			highlightColor: "#3b82f6",
			highlightOpacity: 40,
			hideWhenIdle: false,
			idleTimeout: 3,
			motionBlur: 0,
			clickBounce: 0,
			bounceSpeedMs: 220,
			sway: 0,
		};
		audioSettings = {
			volume: 100,
			muted: false,
			systemVolume: 100,
			systemMuted: false,
			micVolume: 100,
			micMuted: false,
			fadeIn: 0,
			fadeOut: 0,
			normalizeLoudness: false,
		};
		cameraOverlay = {
			enabled: false,
			mirror: true,
			shape: "rounded",
			cornerRadius: 0.16,
			animationPreset: "soft",
			defaultPlacement: cameraPlacementFromPreset("bottom-right"),
			motionSegments: [],
			keyframes: [],
			keyframeEasing: { ...EASE_IN_OUT },
			shadow: 0.35,
			zoomFollow: true,
			zoomFollowStrength: 0.6,
			zoomFollowDuration: 0.4,
			zoomFollowEasing: { ...EASE_IN_OUT },
		};
		exportQuality = "source";
		exportSpeed = "balanced";
		exportFps = null;
		exportFpsDefaulted = false;
		undoStack = [];
		redoStack = [];
	}

	/**
	 * Add a removed range. Returns the new cut id, or null if the range is
	 * too short to be meaningful. Each call is its own undo entry; callers
	 * accepting several silence suggestions at once should batch with their
	 * own `pushUndoState` and use the lower-level array if needed.
	 */
	function addCut(start: number, end: number, source: CutSource = "silence"): string | null {
		if (end - start <= 0.01) return null;
		pushUndoState();
		const cut: TimelineCut = { id: generateId(), start, end, source };
		cuts = [...cuts, cut].sort((a, b) => a.start - b.start);
		return cut.id;
	}

	function removeCut(id: string) {
		if (!cuts.some((c) => c.id === id)) return;
		pushUndoState();
		cuts = cuts.filter((c) => c.id !== id);
		if (selectedCutId === id) selectedCutId = null;
	}

	function clearCuts() {
		if (cuts.length === 0) return;
		pushUndoState();
		cuts = [];
	}

	/**
	 * Resize a cut. Does NOT push undo; callers (the cut lane's drag
	 * handlers) own coalescing via `pushUndoStateCoalesced` so a whole drag
	 * is one undo entry.
	 */
	function updateCut(id: string, start: number, end: number) {
		cuts = cuts.map((c) => (c.id === id ? { ...c, start, end } : c));
	}

	/**
	 * Merge overlapping or touching cuts into one, keeping the earliest id so
	 * the lane's keyed `{#each}` stays stable. Called at the end of a
	 * create/resize drag, never mid-drag, which would yank the dragged card.
	 */
	function mergeCuts() {
		const sorted = [...cuts].sort((a, b) => a.start - b.start);
		const merged: TimelineCut[] = [];
		for (const c of sorted) {
			const last = merged[merged.length - 1];
			if (last && c.start <= last.end + 0.001) {
				last.end = Math.max(last.end, c.end);
				if (c.source === "manual") last.source = "manual";
			} else {
				merged.push({ ...c });
			}
		}
		cuts = merged;
	}

	/** The clip's effective kept bounds [start, end] in original seconds. */
	function clipBounds(): { start: number; end: number } {
		const d = metadata?.duration ?? 0;
		return {
			start: Math.max(0, trimStart),
			end: trimEnd > 0 ? Math.min(trimEnd, d) : d,
		};
	}

	// A cut applies only when its source flag allows it and the lane is on; a disabled flag preserves the edits.
	function cutFlagAllows(c: TimelineCut): boolean {
		return c.source === "silence" ? experimentalStore.silenceDetection : true;
	}
	/** Cuts that actually apply right now (flag-gated + lane-enabled). */
	// $derived, not per-read: the waveform lane's xOf rebuilt this thousands of times per zoom frame.
	const cutsMemo = $derived.by<TimelineCut[]>(() =>
		cutsEnabled ? cuts.filter(cutFlagAllows) : [],
	);
	function effectiveCutList(): TimelineCut[] {
		return cutsMemo;
	}
	function activeSplitPoints(): number[] {
		return splitPoints;
	}

	/** The current clip's kept segments: trim − active cuts, subdivided by
	 * active splits. Drives both the timeline display and the edit math, so the
	 * two never disagree. */
	const segmentsMemo = $derived.by<Segment[]>(() => {
		const { start, end } = clipBounds();
		return deriveSegments({
			trimStart: start,
			trimEnd: end,
			cuts: cutsMemo,
			splitPoints,
		});
	});
	function currentSegments(): Segment[] {
		return segmentsMemo;
	}

	/** The timeline axis: the KEPT clip only (trimmed head/tail collapse away,
	 * Cap-style), with each kept segment warped by its per-segment speed and cuts
	 * closed to seams. `output 0 == inPoint`; the clip fills the track from the
	 * left. Every lane, the playhead, and the preview clock position against this.
	 * At all-1× speeds it's the cut translation map restricted to [inPoint,outPoint]. */
	// Playback and export position against the KEPT axis; `timeMapMemo` swaps in the full recording mid trim-drag.
	const keptTimeMapMemo = $derived.by(() =>
		timeMapFromSegments(segmentsMemo, buildSpeedOf(segmentsMemo, segmentSpeeds)),
	);

	const timeMapMemo = $derived.by(() => {
		// While trimming, un-collapse onto the full recording so the handle can reveal the trimmed head and tail.
		if (isTrimming) {
			const segs = segmentsMemo;
			const { start, end } = clipBounds();
			return displayTimeMap({
				trimStart: start,
				trimEnd: end,
				durationSec: metadata?.duration ?? end,
				segments: segs,
				cuts: cutsMemo,
				speedOf: buildSpeedOf(segs, segmentSpeeds),
			});
		}
		return keptTimeMapMemo;
	});
	function currentTimeMap() {
		return timeMapMemo;
	}

	// Lanes render against this; playback and export never read it, so seeking stays gapless.
	const renderMap = $derived(showCutGaps ? buildGapMap(timeMapMemo) : timeMapMemo);
	// OUTPUT axis (where music and voice clips live) to the render axis and back; identity when gaps are off.
	function outputToRenderSec(outputSec: number): number {
		if (!showCutGaps) return outputSec;
		return originalToOutput(renderMap, outputToOriginal(timeMapMemo, outputSec));
	}
	function renderSecToOutputSec(renderSec: number): number {
		if (!showCutGaps) return renderSec;
		return originalToOutput(timeMapMemo, outputToOriginal(renderMap, renderSec));
	}

	/** Speed of the segment anchored at original `start` (1 when unset). */
	function segmentSpeedAtStart(start: number): number {
		return speedAtAnchor(segmentSpeeds, start);
	}

	/** Speed of the segment that CONTAINS original time `t` (1 when none / unset).
	 * The legacy `<video>` preview reads this to set `playbackRate` per segment,
	 * since that path plays at the element's native rate rather than the warped
	 * output clock. Tolerant at seams: forward-biased onto the next segment. */
	function segmentSpeedAtTime(t: number): number {
		return speedAtTime(currentSegments(), segmentSpeeds, t);
	}

	/** Set the speed of the segment anchored at original `start`. Coalesced into
	 * one undo entry per anchor while a slider drags; orphaned anchors are pruned. */
	function setSegmentSpeed(start: number, speed: number) {
		pushUndoStateCoalesced(`segment-speed-${start.toFixed(3)}`, 400);
		const next = upsertSegmentSpeed(segmentSpeeds, start, speed);
		segmentSpeeds = pruneSegmentSpeeds(next, currentSegments());
		isDirty = true;
	}

	/** The scene animation anchored at original `start` (null when unset). */
	function segmentAnimAtStart(start: number): SegmentAnim | null {
		return animAtAnchor(segmentAnims, start);
	}

	/** Set or clear (spec = null) the entrance/exit animation of the segment
	 * anchored at original `start`. Coalesced into one undo entry per anchor+side
	 * while presets/sliders change; orphaned anchors are pruned. */
	function setSegmentAnim(start: number, side: "in" | "out", spec: SceneAnimSpec | null) {
		pushUndoStateCoalesced(`segment-anim-${side}-${start.toFixed(3)}`, 400);
		const next = upsertSegmentAnim(segmentAnims, start, side, spec);
		segmentAnims = pruneSegmentAnims(next, currentSegments());
		isDirty = true;
	}

	/** Set the project-wide scene-animation motion style, restyling every existing
	 * animation to match (so the dial visibly re-tones the whole video). */
	function setMotionTone(tone: MotionTone) {
		if (tone === motionTone) return;
		pushUndoState();
		motionTone = tone;
		segmentAnims = retuneAnimsForTone(segmentAnims, tone);
		isDirty = true;
	}

	/** Set the transition across the seam between the segments anchored at
	 * `leftStart` and `rightStart`: a matched exit+entrance styled by the current
	 * motion tone. One undo entry for the pair. */
	function setSeamTransition(leftStart: number, rightStart: number, kind: SeamTransition) {
		pushUndoState();
		const next = applySeamTransition(segmentAnims, leftStart, rightStart, kind, motionTone);
		segmentAnims = pruneSegmentAnims(next, currentSegments());
		isDirty = true;
	}

	/** The transition currently spanning that seam ("none" / a push / "custom"). */
	function seamTransitionAt(leftStart: number, rightStart: number): SeamTransition | "custom" {
		return readSeamTransition(segmentAnims, leftStart, rightStart);
	}

	/** Split the clip at original time `t`. Returns true if a split was added. */
	/** Whether a split at `t` would land. Goes through `planSplit` so a disabled
	 *  Split button can never disagree with what pressing it would do. */
	function canSplitAt(t: number): boolean {
		const { start, end } = clipBounds();
		return (
			planSplit(t, {
				trimStart: start,
				trimEnd: end,
				cuts: effectiveCutList(),
				splitPoints,
			}) !== null
		);
	}

	function splitAt(t: number): boolean {
		const { start, end } = clipBounds();
		const next = planSplit(t, {
			trimStart: start,
			trimEnd: end,
			cuts: effectiveCutList(),
			splitPoints,
		});
		if (!next) return false;
		pushUndoState();
		splitPoints = next;
		selectedClipStart = null;
		return true;
	}

	/** Remove the split marker at (≈) original time `t`, rejoining the clips. */
	function removeSplit(t: number) {
		const next = splitPoints.filter((p) => Math.abs(p - t) > 1e-4);
		if (next.length === splitPoints.length) return;
		pushUndoState();
		splitPoints = next;
	}

	function clearSplits() {
		if (splitPoints.length === 0) return;
		pushUndoState();
		splitPoints = [];
	}

	/**
	 * Carry a segment's anchored settings across a boundary move. Speeds and
	 * scene animations are keyed by the segment's ORIGINAL start, so without this
	 * a roll/slide/slip silently drops them (`prune*` treats the anchor as
	 * orphaned the moment the start it names no longer exists).
	 */
	function reanchorSegment(from: number, to: number) {
		if (Math.abs(from - to) <= 1e-4) return;
		const move = <T extends { start: number }>(list: readonly T[]): T[] =>
			list
				.map((it) => (Math.abs(it.start - from) <= 1e-4 ? { ...it, start: to } : { ...it }))
				.sort((a, b) => a.start - b.start);
		segmentSpeeds = move(segmentSpeeds);
		segmentAnims = move(segmentAnims);
	}

	/**
	 * Roll the split at `from` to `to`: the left segment's end and the right
	 * segment's start move together, total length unchanged. Returns false if no
	 * split sits at `from`. Does NOT push undo — the clip bar's drag handler owns
	 * coalescing, like `updateCut`.
	 */
	function moveSplit(from: number, to: number): boolean {
		const index = splitPoints.findIndex((p) => Math.abs(p - from) <= 1e-4);
		if (index === -1) return false;
		const next = [...splitPoints];
		next[index] = to;
		splitPoints = next.sort((a, b) => a - b);
		reanchorSegment(from, to);
		return true;
	}

	/**
	 * Slide a removed range as a unit, so the blocks either side of it grow and
	 * shrink by the same amount and the output length never changes. Callers own
	 * undo coalescing.
	 */
	function slideCut(id: string, start: number, end: number) {
		const cut = cuts.find((c) => c.id === id);
		if (!cut) return;
		// The following segment starts where the cut ends, so its anchor rides along.
		reanchorSegment(cut.end, end);
		cuts = cuts
			.map((c) => (c.id === id ? { ...c, start, end } : c))
			.sort((a, b) => a.start - b.start);
	}

	/**
	 * Slip a block: its source window shifts inside its slot while it stays put
	 * on the output axis, absorbed by the removed ranges either side. Callers own
	 * undo coalescing.
	 */
	function slipSegment(p: {
		from: number;
		to: number;
		before: { id: string; start: number; end: number };
		after: { id: string; start: number; end: number };
	}) {
		reanchorSegment(p.from, p.to);
		cuts = cuts
			.map((c) => {
				if (c.id === p.before.id) return { ...c, start: p.before.start, end: p.before.end };
				if (c.id === p.after.id) return { ...c, start: p.after.start, end: p.after.end };
				return c;
			})
			.sort((a, b) => a.start - b.start);
	}

	/**
	 * Ripple-delete the segment containing original time `t`: the segment's
	 * range becomes a manual cut and the gap closes via the cut time-map. Pruned
	 * split points that bordered it are dropped. Returns the original-time
	 * "join" (the first kept frame after the deletion) to park the playhead on,
	 * or null when there's nothing to delete (only one segment, or `t` is not in
	 * a segment): the whole recording can't be removed this way.
	 */
	function deleteSegmentAt(t: number): number | null {
		const segs = currentSegments();
		if (segs.length <= 1) return null;
		const seg = segmentAt(segs, t);
		if (!seg) return null;
		pushUndoState();
		const plan = planDeleteSegment(seg, splitPoints);
		splitPoints = plan.splitPoints;
		cuts = [
			...cuts,
			{
				id: generateId(),
				start: plan.cut.start,
				end: plan.cut.end,
				source: "manual" as CutSource,
			},
		].sort((a, b) => a.start - b.start);
		selectedClipStart = null;
		// `seg.end` is the first kept frame after the removed range; `seg.start` would land inside the new cut.
		return seg.end;
	}

	/** Record a dismissed silence range so detection won't suggest it again. */
	function dismissSilence(start: number, end: number) {
		dismissedSilences = [...dismissedSilences, { start, end }];
		isDirty = true;
	}

	/** Wipe all dismissed silence ranges so the next detection pass surfaces
	 *  every candidate again. Used by the popover's "Reset dismissed" button
	 *  when the user wants to reconsider previously-rejected suggestions. */
	function clearDismissedSilences() {
		if (dismissedSilences.length === 0) return;
		dismissedSilences = [];
		isDirty = true;
	}

	function toRenderState(): EditorRenderState {
		return {
			trimStart,
			trimEnd,
			outputAspect,
			lastAppliedPresetId,
			backgroundType,
			// `ext:` ids resolve to the pack's absolute path; built-in values pass through unchanged.
			backgroundValue: resolveBackgroundWireValue(backgroundValue),
			backgroundBlur,
			padding,
			borderRadius,
			cursorEnabled: cursorSettings.enabled,
			cursorSize: cursorSettings.size,
			cursorStyle: cursorSettings.style,
			cursorSmoothing: cursorSettings.smoothing,
			cursorSnapToClicks: cursorSettings.snapToClicks,
			cursorSnapWindowMs: cursorSettings.snapWindowMs,
			cursorHighlightClicks: cursorSettings.highlightClicks,
			cursorHighlightColor: cursorSettings.highlightColor,
			cursorHighlightOpacity: cursorSettings.highlightOpacity,
			cursorHideWhenIdle: cursorSettings.hideWhenIdle,
			cursorIdleTimeout: cursorSettings.idleTimeout,
			cursorMotionBlur: cursorSettings.motionBlur,
			cursorClickBounce: cursorSettings.clickBounce,
			cursorBounceSpeedMs: cursorSettings.bounceSpeedMs,
			cursorSway: cursorSettings.sway,
			zoomRegions: zoomRegions.map((region) => ({
				id: region.id,
				start: region.start,
				end: region.end,
				scale: region.scale,
				easeIn: region.easeIn,
				easeOut: region.easeOut,
				rampIn: region.rampIn,
				rampOut: region.rampOut,
				centerX: region.centerX,
				centerY: region.centerY,
				motionBlur: region.motionBlur,
				source: region.source,
				hidden: region.hidden ?? false,
			})),
			autoZoomApplied,
			autoZoomEnabled,
			cuts: cuts.map((cut) => ({ ...cut })),
			splitPoints: [...splitPoints],
			// Prune orphaned anchors on save so the section diffs cleanly.
			segmentSpeeds: pruneSegmentSpeeds(segmentSpeeds, currentSegments()),
			segmentAnims: pruneSegmentAnims(segmentAnims, currentSegments()),
			motionTone,
			cutsEnabled,
			focusEnabled,
			annotationsEnabled: !annotationsGloballyHidden,
			dismissedSilences: dismissedSilences.map((d) => ({ ...d })),
			cursorMotionEasing,
			annotations: annotations.map((annotation) => ({ ...annotation })),
			shadow: { ...shadow },
			audioSettings: { ...audioSettings },
			musicClips: musicClips.map((c) => ({ ...c, source: { ...c.source } })),
			transcript,
			captionStyle: { ...captionStyle },
			cameraOverlay: {
				...cameraOverlay,
				defaultPlacement: { ...cameraOverlay.defaultPlacement },
				motionSegments: cameraOverlay.motionSegments.map((segment) => ({
					...segment,
				})),
				keyframes: cameraOverlay.keyframes.map((k) => ({
					atSec: k.atSec,
					placement: { ...k.placement },
				})),
				keyframeEasing: { ...cameraOverlay.keyframeEasing },
			},
			layoutMode,
		};
	}

	function loadRenderState(state: Partial<EditorRenderState>) {
		trimStart = state.trimStart ?? 0;
		trimEnd = state.trimEnd ?? metadata?.duration ?? 0;
		outputAspect = state.outputAspect ?? "source";
		lastAppliedPresetId = state.lastAppliedPresetId ?? null;
		backgroundType = state.backgroundType ?? "color";
		// Retired stock presets forward to their replacement, so an old project still highlights a swatch.
		backgroundValue = migrateBackgroundValue(state.backgroundValue ?? "#111111");
		backgroundBlur = state.backgroundBlur ?? 0;
		padding = normalizeFramePaddingPercent(state.padding ?? 0, metadata);
		borderRadius = state.borderRadius ?? 0;
		cursorSettings = {
			...cursorSettings,
			enabled: state.cursorEnabled ?? cursorSettings.enabled,
			size: state.cursorSize ?? cursorSettings.size,
			style: state.cursorStyle ?? cursorSettings.style,
			smoothing: state.cursorSmoothing ?? cursorSettings.smoothing,
			snapToClicks: state.cursorSnapToClicks ?? cursorSettings.snapToClicks,
			snapWindowMs: state.cursorSnapWindowMs ?? cursorSettings.snapWindowMs,
			highlightClicks: state.cursorHighlightClicks ?? cursorSettings.highlightClicks,
			highlightColor: state.cursorHighlightColor ?? cursorSettings.highlightColor,
			highlightOpacity: state.cursorHighlightOpacity ?? cursorSettings.highlightOpacity,
			hideWhenIdle: state.cursorHideWhenIdle ?? cursorSettings.hideWhenIdle,
			idleTimeout: state.cursorIdleTimeout ?? cursorSettings.idleTimeout,
			motionBlur: state.cursorMotionBlur ?? cursorSettings.motionBlur,
			clickBounce: state.cursorClickBounce ?? cursorSettings.clickBounce,
			bounceSpeedMs: state.cursorBounceSpeedMs ?? cursorSettings.bounceSpeedMs,
			sway: state.cursorSway ?? cursorSettings.sway,
		};
		zoomRegions = (state.zoomRegions ?? []).map((region) => ({
			id: region.id ?? generateId(),
			start: region.start,
			end: region.end,
			scale: region.scale,
			easeIn: region.easeIn ?? { ...EASE },
			easeOut: region.easeOut ?? { ...EASE },
			rampIn: region.rampIn ?? DEFAULT_ZOOM_RAMP,
			rampOut: region.rampOut ?? DEFAULT_ZOOM_RAMP,
			centerX: region.centerX ?? DEFAULT_ZOOM_CENTER,
			centerY: region.centerY ?? DEFAULT_ZOOM_CENTER,
			motionBlur: region.motionBlur ?? DEFAULT_ZOOM_MOTION_BLUR,
			source: region.source ?? "manual",
			hidden: region.hidden ?? false,
		}));
		// Legacy projects predate the flags; treat them as processed so zooms aren't scattered over finished edits.
		autoZoomEnabled = state.autoZoomEnabled ?? true;
		autoZoomApplied = state.autoZoomApplied ?? state.zoomRegions !== undefined;
		cuts = (state.cuts ?? []).map((c) => ({
			id: c.id ?? generateId(),
			start: c.start,
			end: c.end,
			source: c.source ?? "silence",
		}));
		dismissedSilences = (state.dismissedSilences ?? []).map((d) => ({
			start: d.start,
			end: d.end,
		}));
		cutsEnabled = state.cutsEnabled ?? true;
		splitPoints = [...(state.splitPoints ?? [])];
		segmentSpeeds = (state.segmentSpeeds ?? []).map((o) => ({ ...o }));
		segmentAnims = (state.segmentAnims ?? []).map((o) => ({ ...o }));
		motionTone = state.motionTone ?? "balanced";
		focusEnabled = state.focusEnabled ?? true;
		shadow = state.shadow ?? shadow;
		musicClips = (state.musicClips ?? []).map((c) => ({ ...c, source: { ...c.source } }));
		// Backward-compat (see comment in loadRenderState).
		if (state.audioSettings) {
			const loaded = state.audioSettings;
			audioSettings = {
				volume: loaded.volume,
				muted: loaded.muted,
				systemVolume: loaded.systemVolume ?? loaded.volume,
				systemMuted: loaded.systemMuted ?? loaded.muted,
				micVolume: loaded.micVolume ?? loaded.volume,
				micMuted: loaded.micMuted ?? loaded.muted,
				fadeIn: loaded.fadeIn,
				fadeOut: loaded.fadeOut,
				normalizeLoudness: loaded.normalizeLoudness ?? false,
			};
		} else {
			audioSettings = audioSettings;
		}
		transcript = state.transcript ?? null;
		captionStyle = state.captionStyle
			? { ...DEFAULT_CAPTION_STYLE, ...state.captionStyle }
			: { ...DEFAULT_CAPTION_STYLE };
		// Phase 1 defaults (bottom-right, 16%); the fallbacks below keep an older project's top-right 22% placement.
		const fallbackPlacement = cameraPlacementFromPreset("bottom-right");
		// Clamped: a live capture can record a placement outside the frame.
		const loadedPlacement = clampPlacement({
			x: state.cameraOverlay?.defaultPlacement?.x ?? fallbackPlacement.x,
			y: state.cameraOverlay?.defaultPlacement?.y ?? fallbackPlacement.y,
			width: state.cameraOverlay?.defaultPlacement?.width ?? fallbackPlacement.width,
			height: state.cameraOverlay?.defaultPlacement?.height ?? fallbackPlacement.height,
		});
		const loadedKeyframes = (state.cameraOverlay?.keyframes ?? []).map((k) => ({
			atSec: k.atSec,
			placement: { ...k.placement },
		}));
		// Recorded `motionSegments` are folded into keyframes and dropped; authored keyframes win.
		const recordedKeyframes =
			loadedKeyframes.length > 0
				? loadedKeyframes
				: keyframesFromMotionSegments(
						(state.cameraOverlay?.motionSegments ?? []).map((segment) => ({
							...segment,
							easeIn: segment.easeIn ?? { ...EASE },
							easeOut: segment.easeOut ?? { ...EASE },
						})),
						loadedPlacement,
					);
		cameraOverlay = {
			enabled: state.cameraOverlay?.enabled ?? false,
			mirror: state.cameraOverlay?.mirror ?? true,
			shape: state.cameraOverlay?.shape ?? "rounded",
			cornerRadius: state.cameraOverlay?.cornerRadius ?? 0.16,
			animationPreset: state.cameraOverlay?.animationPreset ?? "soft",
			zoomFollow: state.cameraOverlay?.zoomFollow ?? true,
			zoomFollowStrength: state.cameraOverlay?.zoomFollowStrength ?? 0.6,
			zoomFollowDuration: state.cameraOverlay?.zoomFollowDuration ?? 0.4,
			zoomFollowEasing: state.cameraOverlay?.zoomFollowEasing
				? { ...state.cameraOverlay.zoomFollowEasing }
				: { ...EASE_IN_OUT },
			keyframes: recordedKeyframes,
			keyframeEasing: state.cameraOverlay?.keyframeEasing
				? { ...state.cameraOverlay.keyframeEasing }
				: { ...EASE_IN_OUT },
			shadow: state.cameraOverlay?.shadow ?? 0.35,
			defaultPlacement: loadedPlacement,
			motionSegments: [],
		};
		cursorMotionEasing = state.cursorMotionEasing ?? null;
		layoutMode = state.layoutMode ?? layoutMode;
		annotations = (state.annotations ?? []).map((a, idx) => ({
			id: generateId(),
			start: a.start,
			end: a.end,
			rampIn: a.rampIn ?? DEFAULT_ANNOTATION_RAMP,
			rampOut: a.rampOut ?? DEFAULT_ANNOTATION_RAMP,
			easeIn: a.easeIn ?? { ...EASE },
			easeOut: a.easeOut ?? { ...EASE },
			stroke: a.stroke ?? { ...DEFAULT_ANNOTATION_STROKE },
			fill: a.fill ?? DEFAULT_ANNOTATION_FILL,
			kind: a.kind,
			// v2 fields with sane defaults so v1 projects keep loading.
			name: a.name,
			zIndex: a.zIndex ?? idx + 1,
			locked: a.locked ?? false,
			hidden: a.hidden ?? false,
			opacity: a.opacity ?? 1,
			glow: a.glow,
			anchor: a.anchor,
		}));
		annotationZSeq = annotations.length + 1;
		selectedAnnotationId = null;
		annotationTool = null;
		timelineTool = "select";
		hoveredAnnotationId = null;
		// Hidden only when explicitly disabled; absent (older projects) or true means visible.
		annotationsGloballyHidden = state.annotationsEnabled === false;
		// A freshly loaded document matches on-disk state, so no unsaved edits.
		isDirty = false;
		// Anchor `revertToSaved` to the just-loaded state.
		savedSnapshot = getSettingsSnapshot();
	}

	return {
		// Getters (reactive reads)
		get videoPath() {
			return videoPath;
		},
		set videoPath(v: string) {
			videoPath = v;
		},

		get cursorPath() {
			return cursorPath;
		},
		set cursorPath(v: string | null) {
			cursorPath = v;
		},

		get recordingPath() {
			return recordingPath;
		},
		set recordingPath(v: string | null) {
			recordingPath = v;
		},

		get audioPath() {
			return audioPath;
		},
		set audioPath(v: string | null) {
			audioPath = v;
		},

		get microphonePath() {
			return microphonePath;
		},
		set microphonePath(v: string | null) {
			microphonePath = v;
		},

		get metadata() {
			return metadata;
		},
		set metadata(v: VideoMetadata | null) {
			metadata = v;
			// 60fps for >60fps recordings roughly halves export time; seeded once so a later user choice stands.
			if (v) {
				if (!exportFpsDefaulted && v.fps > 60.5) exportFps = 60;
				exportFpsDefaulted = true;
			}
		},

		get thumbnailStrip() {
			return thumbnailStrip;
		},
		set thumbnailStrip(v: string[]) {
			thumbnailStrip = v;
		},

		get waveform() {
			return waveform;
		},
		set waveform(v: number[]) {
			waveform = v;
		},

		get currentTime() {
			return currentTime;
		},
		set currentTime(v: number) {
			currentTime = v;
		},

		/** Register the transport seek (the <video>/clock owner). Returns an
		 *  unsubscribe to call on teardown. */
		registerSeekHandler(fn: (time: number) => void) {
			seekHandler = fn;
			return () => {
				if (seekHandler === fn) seekHandler = null;
			};
		},
		/** Move the playhead AND the playback transport to `time`. Use this for
		 *  any seek that originates outside the player (transcript, chapters, …)
		 *  so it lands whether paused or playing. */
		seek(time: number) {
			currentTime = time;
			seekHandler?.(time);
		},

		get timelineTool() {
			return timelineTool;
		},
		set timelineTool(v: TimelineTool) {
			timelineTool = v;
		},

		/** Register the timeline's keyboard-command handlers. Returns an
		 *  unsubscribe for teardown, mirroring registerSeekHandler. */
		registerTimelineCommands(cmds: TimelineCommands) {
			timelineCommands = cmds;
			return () => {
				if (timelineCommands === cmds) timelineCommands = null;
			};
		},
		get timelineCommands() {
			return timelineCommands;
		},

		get isPlaying() {
			return isPlaying;
		},
		set isPlaying(v: boolean) {
			isPlaying = v;
		},

		// Setters do NOT push undo: Timeline handlers own coalescing, so a drag is one entry, not one per frame.
		get trimStart() {
			return trimStart;
		},
		set trimStart(v: number) {
			trimStart = v;
			isDirty = true;
		},

		get trimEnd() {
			return trimEnd;
		},
		set trimEnd(v: number) {
			trimEnd = v;
			isDirty = true;
		},

		// `outPoint` resolves the legacy `0 = unset` sentinel, so callers skip the `trimEnd || duration` dance.
		get inPoint() {
			return Math.max(0, trimStart);
		},
		get outPoint() {
			const d = metadata?.duration ?? 0;
			return trimEnd > 0 ? Math.min(trimEnd, d) : d;
		},
		get clipDuration() {
			const d = metadata?.duration ?? 0;
			const out = trimEnd > 0 ? Math.min(trimEnd, d) : d;
			return Math.max(0, out - Math.max(0, trimStart));
		},

		get backgroundType() {
			return backgroundType;
		},
		set backgroundType(v: BackgroundType) {
			pushUndoState();
			backgroundType = v;
		},

		get backgroundValue() {
			return backgroundValue;
		},
		set backgroundValue(v: string) {
			pushUndoState();
			backgroundValue = v;
		},

		get backgroundBlur() {
			return backgroundBlur;
		},
		set backgroundBlur(v: number) {
			backgroundBlur = v;
		},

		get padding() {
			return padding;
		},
		set padding(v: number) {
			padding = clampFramePaddingPercent(v);
		},

		get borderRadius() {
			return borderRadius;
		},
		set borderRadius(v: number) {
			borderRadius = v;
		},

		get shadow() {
			return shadow;
		},
		set shadow(v: ShadowSettings) {
			shadow = v;
		},

		get musicClips() {
			return musicClips;
		},
		get musicOnlyClips() {
			return musicOnlyClips;
		},
		get voiceClips() {
			return voiceClips;
		},
		get audioDetached() {
			return audioDetached;
		},
		get canDetachAudio() {
			return canDetachAudio;
		},
		detachRecordingAudio,
		reattachRecordingAudio,
		addMusicClip,
		updateMusicClip,
		removeMusicClip,
		splitMusicClip,
		selectMusicClip,
		get selectedMusicClipId() {
			return selectedMusicClipId;
		},
		set selectedMusicClipId(v: string | null) {
			selectMusicClip(v);
		},

		get layoutMode() {
			return layoutMode;
		},
		set layoutMode(v: LayoutMode) {
			pushUndoState();
			layoutMode = v;
		},
		get outputAspect() {
			return outputAspect;
		},
		set outputAspect(v: OutputAspect) {
			pushUndoState();
			outputAspect = v;
		},
		get lastAppliedPresetId() {
			return lastAppliedPresetId;
		},
		set lastAppliedPresetId(v: string | null) {
			lastAppliedPresetId = v;
		},

		get zoomRegions() {
			return zoomRegions;
		},

		// `cuts` is the raw stored list; `effectiveCuts` is the flag- and lane-gated subset that actually applies.
		get cuts() {
			return cuts;
		},
		get effectiveCuts() {
			return effectiveCutList();
		},
		get cutDuration() {
			return totalCutDuration(cuts);
		},
		get dismissedSilences() {
			return dismissedSilences;
		},

		// Lane toggles bypass the effect in preview and export while keeping the underlying data intact.
		get cutsEnabled() {
			return cutsEnabled;
		},
		set cutsEnabled(v: boolean) {
			cutsEnabled = v;
			isDirty = true;
			log.info("feature", "toggled", { feature: "cuts", enabled: v });
		},

		// `segments` is derived (trim minus active cuts, sliced by splits); both empty when timeline editing is off.
		get segments() {
			return currentSegments();
		},
		get splitPoints() {
			return activeSplitPoints();
		},
		get segmentSpeeds() {
			return segmentSpeeds;
		},
		// Reduces to the cut translation map at 1x, and un-collapses to the full recording while `isTrimming`.
		get timeMap() {
			return currentTimeMap();
		},
		/** The kept axis, never the trim-drag display axis. What the export
		 *  replays and what playback positions against. */
		get keptTimeMap() {
			return keptTimeMapMemo;
		},
		get renderMap() {
			return renderMap;
		},
		outputToRenderSec,
		renderSecToOutputSec,
		get showCutGaps() {
			return showCutGaps;
		},
		set showCutGaps(v: boolean) {
			showCutGaps = v;
		},
		get isTrimming() {
			return isTrimming;
		},
		set isTrimming(v: boolean) {
			isTrimming = v;
		},
		segmentSpeedAt: segmentSpeedAtStart,
		segmentSpeedAtTime,
		setSegmentSpeed,
		get segmentAnims() {
			return segmentAnims;
		},
		segmentAnimAt: segmentAnimAtStart,
		setSegmentAnim,
		get motionTone() {
			return motionTone;
		},
		setMotionTone,
		setSeamTransition,
		seamTransitionAt,
		// Setters route through the exclusive selectors, so every call site gets one-selection-at-a-time.
		get selectedClipStart() {
			return selectedClipStart;
		},
		set selectedClipStart(v: number | null) {
			selectClip(v);
		},

		get selection() {
			return selection;
		},
		clearSelection,
		deleteSelection,
		get focusEnabled() {
			return focusEnabled;
		},
		set focusEnabled(v: boolean) {
			focusEnabled = v;
			isDirty = true;
			log.info("feature", "toggled", { feature: "focus", enabled: v });
		},

		get autoZoomEnabled() {
			return autoZoomEnabled;
		},
		set autoZoomEnabled(v: boolean) {
			autoZoomEnabled = v;
			isDirty = true;
			log.info("feature", "toggled", { feature: "autoZoom", enabled: v });
		},

		get autoZoomApplied() {
			return autoZoomApplied;
		},
		set autoZoomApplied(v: boolean) {
			autoZoomApplied = v;
			isDirty = true;
		},

		get cursorSamplesRaw() {
			return cursorSamplesRaw;
		},
		set cursorSamplesRaw(v: CursorSampleLike[]) {
			cursorSamplesRaw = v;
		},

		get cursorIdlePeriods() {
			return cursorIdlePeriods;
		},
		set cursorIdlePeriods(v: { startUs: number; endUs: number }[]) {
			cursorIdlePeriods = v;
		},

		get selectedZoomRegionId() {
			return selectedZoomRegionId;
		},
		set selectedZoomRegionId(v: string | null) {
			selectZoomRegion(v);
		},

		get selectedCutId() {
			return selectedCutId;
		},
		set selectedCutId(v: string | null) {
			selectCut(v);
		},

		get activePanel() {
			return activePanel;
		},
		set activePanel(v: PanelTab) {
			activePanel = v;
		},

		get timeMode() {
			return timeMode;
		},
		set timeMode(v: TimeMode) {
			timeMode = v;
		},

		get cursorMotionEasing() {
			return cursorMotionEasing;
		},
		set cursorMotionEasing(v: Easing | null) {
			pushUndoState();
			cursorMotionEasing = v;
		},

		get annotations() {
			return annotations;
		},
		get annotationsByZ() {
			return annotationsByZOrdered;
		},
		get selectedAnnotationId() {
			return selectedAnnotationId;
		},
		set selectedAnnotationId(v: string | null) {
			selectAnnotation(v);
		},
		get annotationTool() {
			return annotationTool;
		},
		set annotationTool(v: AnnotationKindName | null) {
			annotationTool = v;
		},
		get hoveredAnnotationId() {
			return hoveredAnnotationId;
		},
		set hoveredAnnotationId(v: string | null) {
			hoveredAnnotationId = v;
		},
		get annotationsGloballyHidden() {
			return annotationsGloballyHidden;
		},
		set annotationsGloballyHidden(v: boolean) {
			annotationsGloballyHidden = v;
			log.info("feature", "toggled", { feature: "annotations", enabled: !v });
		},
		get annotationSnapEnabled() {
			return annotationSnapEnabled;
		},
		set annotationSnapEnabled(v: boolean) {
			annotationSnapEnabled = v;
		},

		get cursorSettings() {
			return cursorSettings;
		},
		set cursorSettings(v: CursorSettings) {
			cursorSettings = v;
		},

		get audioSettings() {
			return audioSettings;
		},
		set audioSettings(v: AudioSettings) {
			audioSettings = v;
		},

		get transcript() {
			return transcript;
		},
		set transcript(v: Transcript | null) {
			transcript = v;
			isDirty = true;
		},
		/** Duration (s) of the audio the transcript was timed against; set on load
		 *  so `captionTranscript` can correct the audio-vs-video CFR drift. */
		get captionAudioDurationSec() {
			return captionAudioDurationSec;
		},
		set captionAudioDurationSec(v: number | null) {
			captionAudioDurationSec = v;
		},
		/** The transcript rescaled onto the video/timeMap time axis. EVERY caption
		 *  surface (preview overlay, export burn, sidecar) reads THIS, so they stay
		 *  in sync with the frames instead of drifting toward the end. */
		get captionTranscript() {
			return captionTranscriptMemo;
		},
		get captionStyle() {
			return captionStyle;
		},
		set captionStyle(v: CaptionStyle) {
			captionStyle = v;
			isDirty = true;
		},
		updateCaptionStyle(updates: Partial<CaptionStyle>) {
			captionStyle = { ...captionStyle, ...updates };
			isDirty = true;
		},

		get cameraOverlay() {
			return cameraOverlay;
		},
		set cameraOverlay(v: CameraOverlaySettings) {
			cameraOverlay = v;
		},

		get exportFormat() {
			return exportFormat;
		},
		set exportFormat(v: ExportFormat) {
			exportFormat = v;
		},

		get exportQuality() {
			return exportQuality;
		},
		set exportQuality(v: ExportQuality) {
			exportQuality = v;
		},

		get exportSpeed() {
			return exportSpeed;
		},
		set exportSpeed(v: ExportSpeed) {
			exportSpeed = v;
		},

		get exportFps() {
			return exportFps;
		},
		set exportFps(v: number | null) {
			exportFps = v;
		},

		get captionExport() {
			return captionExport;
		},
		set captionExport(v: CaptionExportOptions) {
			captionExport = v;
		},
		updateCaptionExport(updates: Partial<CaptionExportOptions>) {
			captionExport = { ...captionExport, ...updates };
		},

		get gifSettings() {
			return gifSettings;
		},
		set gifSettings(v: GifSettings) {
			gifSettings = v;
		},
		updateGifSettings(updates: Partial<GifSettings>) {
			gifSettings = { ...gifSettings, ...updates };
		},

		get exportProgress() {
			return exportProgress;
		},
		set exportProgress(v: number | null) {
			exportProgress = v;
		},

		get isExporting() {
			return isExporting;
		},
		set isExporting(v: boolean) {
			isExporting = v;
		},

		get timelineZoom() {
			return timelineZoom;
		},
		set timelineZoom(v: number) {
			timelineZoom = v;
		},

		get canUndo() {
			return undoStack.length > 0;
		},
		get canRedo() {
			return redoStack.length > 0;
		},
		// Needs a saved baseline AND divergence; without `isDirty` the button is a no-op that eats an undo slot.
		get canRevert() {
			return isDirty && savedSnapshot !== null;
		},

		get isDirty() {
			return isDirty;
		},
		get lastSavedAt() {
			return lastSavedAt;
		},

		// Methods
		undo,
		redo,
		pushUndoState,
		popUndoState,
		withoutUndo,
		pushUndoStateCoalesced,
		markSaved,
		revertToSaved,
		setBackground,
		setBackgroundLive,
		setCursorMotionEasingLive,
		updateCursorSettings,
		updateAudioSettings,
		updateShadow,
		updateCameraOverlay,
		updateCameraOverlayLive,
		setCameraPlacement,
		setCameraPerCut,
		removeCameraKeyframeNear,
		addZoomRegion,
		addAutoZoomRegion,
		clearAutoZooms,
		removeZoomRegion,
		clearZoomRegions,
		duplicateZoomRegion,
		setZoomRegionHidden,
		updateZoomRegion,
		selectZoomRegion,
		addCut,
		removeCut,
		clearCuts,
		updateCut,
		mergeCuts,
		splitAt,
		canSplitAt,
		removeSplit,
		clearSplits,
		moveSplit,
		slideCut,
		slipSegment,
		deleteSegmentAt,
		dismissSilence,
		clearDismissedSilences,
		addAnnotation,
		updateAnnotation,
		removeAnnotation,
		toggleAnnotationLock,
		toggleAnnotationVisibility,
		renameAnnotation,
		duplicateAnnotation,
		reorderAnnotation,
		setAnnotationZOrder,
		reset,
		toRenderState,
		loadRenderState,
	};
}

export type EditorStore = ReturnType<typeof createEditorStore>;
