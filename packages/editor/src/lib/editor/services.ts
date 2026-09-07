/**
 * The capability boundary between the editor and its host app. Desktop supplies
 * Tauri-backed implementations; web supplies web ones or omits them entirely.
 *
 * An omitted service means the feature HIDES — never that it throws when
 * clicked. Panels gate on `services.x !== undefined`, so the same tree renders
 * a reduced editor on the web without a single platform conditional inside it.
 */

import { getContext, hasContext, setContext } from "svelte";
import type {
	AssetInstallResult,
	CaptionDownloadProgress,
	CaptionModelInfo,
	DeviceCapabilities,
	HydratedAsset,
	InstalledExtension,
	OcrProgress,
	SilenceDetectOptions,
	SilenceSegment,
	TranscribeProgress,
	VideoTextTimeline,
	ZoomSuggestion,
} from "../wire-types";
import type { Transcript, TranscriptSegment, TranscriptWord, VideoMetadata } from "./render-state";

export type {
	CaptionDownloadProgress,
	CaptionModelInfo,
	DeviceCapabilities,
	OcrProgress,
	SilenceDetectOptions,
	SilenceSegment,
	TranscribeProgress,
	Transcript,
	TranscriptSegment,
	TranscriptWord,
	VideoMetadata,
	VideoTextTimeline,
	ZoomSuggestion,
};

/** Turn a stored asset reference into something `fetch`/`<img>` can load: a
 *  filesystem path through the asset protocol on desktop, an object URL on web.
 *  `data:` and `http(s):` refs must pass through untouched. */
export type ResolveAssetUrl = (ref: string) => string;

export interface PickFileOptions {
	/** Extensions without the dot, e.g. `["png", "jpg"]`. */
	accept: string[];
	title?: string;
}

export interface CaptionFileService {
	/** Persist a transcript as a subtitle sidecar: a save dialog + native write
	 *  on desktop, a download on web. Already mapped to output time by the caller. */
	exportSidecar(transcript: Transcript, format: "srt" | "vtt"): Promise<void>;
	/** Read a user-supplied `.srt`/`.vtt`. Omit ⇒ no import affordance (desktop
	 *  generates instead). Resolves null when the user cancels. */
	importSidecar?(): Promise<Transcript | null>;
}

export interface TranscriptionService {
	capabilities(): Promise<DeviceCapabilities>;
	listModels(): Promise<CaptionModelInfo[]>;
	downloadModel(id: string, onProgress?: (p: CaptionDownloadProgress) => void): Promise<void>;
	deleteModel(id: string): Promise<void>;
	/** False when every candidate path lacks an audio stream, so "Generate" can
	 *  gate itself instead of failing mid-run. */
	hasTranscribableAudio(paths: (string | null | undefined)[]): Promise<boolean>;
	transcribe(args: {
		audioPath?: string | null;
		microphonePath?: string | null;
		modelId: string;
		language?: string | null;
		onPhase?: (p: TranscribeProgress) => void;
	}): Promise<Transcript>;
	/** Stop the in-flight `transcribe`, which then rejects with
	 *  {@link TRANSCRIBE_CANCELLED}. Omit ⇒ no cancel affordance. */
	cancel?(): Promise<void>;
}

/** Rejection message a host uses to report a user cancel rather than a failure.
 *  Mirrored by `CANCELLED_MSG` in `transcription/cancel.rs`. */
export const TRANSCRIBE_CANCELLED = "transcription cancelled";

export interface AnalysisService {
	detectSilence(args: {
		audioPath?: string | null;
		microphonePath?: string | null;
		cursorPath?: string | null;
		options?: SilenceDetectOptions;
	}): Promise<SilenceSegment[]>;
	suggestZoomRegions(cursorPath: string): Promise<ZoomSuggestion[]>;
}

/** Source-media analysis that needs a native decoder. The WebCodecs filmstrip
 *  (`lib/timeline/filmstrip-source.ts`) covers the timeline without this; only
 *  the waveform has no browser equivalent today. */
export interface MediaAnalysisService {
	waveform(args: {
		audioPath?: string | null;
		microphonePath?: string | null;
		buckets?: number;
	}): Promise<number[]>;
	thumbnails(path: string, count: number): Promise<string[]>;
	videoMetadata(path: string): Promise<VideoMetadata>;
}

/** On-device asset install/cache. Absent on hosts with no local store, where
 *  wallpapers and fonts are fetched over HTTP instead. */
export interface AssetService {
	/** Download-once + cache a Google Font woff2; resolves to an asset ref. */
	googleFont(family: string, weight: number): Promise<string>;
	/** The same family as a TTF. The engine shapes with rustybuzz, which reads
	 *  neither woff2 nor a `FontFace`, so it needs the file itself. Absent on a
	 *  host with no TTF cache, where engine captions fall back to the default. */
	captionFontFile?(family: string, weight: number): Promise<string>;
	ensureInstalled(manifestUrl: string): Promise<AssetInstallResult>;
	getCachedPath(id: string): Promise<string | null>;
	hydrate(): Promise<HydratedAsset[]>;
}

/** Asset-pack install. Absent ⇒ the Extensions panel is read-only. */
export interface ExtensionService {
	fetchRegistry<T = unknown>(indexUrl: string): Promise<T>;
	install(manifestUrl: string): Promise<InstalledExtension>;
	listInstalled(): Promise<InstalledExtension[]>;
	setEnabled(extId: string, enabled: boolean): Promise<void>;
	uninstall(extId: string): Promise<void>;
}

/**
 * Where a finished export goes. Desktop persists the browser-composited video
 * to a temp file and hands the path to the Rust mux job; web hands back a Blob
 * for download. `enqueue` is the native render queue — absent on web, where the
 * browser compositor is the only engine.
 */
export interface ExportSink {
	deliver(bytes: Uint8Array, suggestedName: string): Promise<string | null>;
	enqueue?(job: unknown): Promise<string[]>;
}

export interface ShellService {
	openFileLocation(path: string): Promise<void>;
	openExternal(url: string): Promise<void>;
}

export interface OcrService {
	readVideoText(args: {
		videoPath: string;
		previews?: boolean;
		includeRanges?: [number, number][];
		onPhase?: (p: OcrProgress) => void;
	}): Promise<VideoTextTimeline>;
	/** Persist an already-serialized read. The service owns the destination
	 *  prompt; `defaultName`'s extension decides the format. */
	exportScreenText(body: string, defaultName: string): Promise<void>;
}

export interface EditorServices {
	resolveAssetUrl: ResolveAssetUrl;
	/** Native picker returning an asset ref. Omit ⇒ `<input type=file>` fallback. */
	pickFile?: (opts: PickFileOptions) => Promise<string | null>;
	/** Omit ⇒ Captions offers import only, no "Auto-generate". */
	transcription?: TranscriptionService;
	/** Subtitle sidecar read/write. Omit ⇒ neither affordance shows. */
	captionFiles?: CaptionFileService;
	/** Omit ⇒ silence review + auto-zoom suggestions hide. */
	analysis?: AnalysisService;
	/** Omit ⇒ Rust waveform/thumbnail strips are skipped. */
	mediaAnalysis?: MediaAnalysisService;
	/** Omit ⇒ Music panel hides; fonts fall back to a direct fetch. */
	assets?: AssetService;
	/** Omit ⇒ the Extensions panel lists but cannot install. */
	extensions?: ExtensionService;
	/** Omit ⇒ "reveal in folder" and credit links hide. */
	shell?: ShellService;
	/** Omit ⇒ the dev OCR panel is unavailable. */
	ocr?: OcrService;
	/** Where a finished export lands. Omit ⇒ export is unavailable. */
	exportSink?: ExportSink;
}

const KEY = Symbol("recast.editor.services");

/**
 * App-scoped fallback for the modules that cannot read Svelte context: pure
 * helpers (`background-source`, `image-import`, `google-fonts`) and the export
 * queue, which outlives the editor component that queued the job.
 */
let current: EditorServices | null = null;

/** Install the services for this editor session. Call once, during the host
 *  component's init, before anything reads them. */
export function setEditorServices(services: EditorServices): void {
	current = services;
	setContext(KEY, services);
}

/** Install outside a component (tests, the export queue). Returns a restore fn. */
export function setEditorServicesForApp(services: EditorServices): () => void {
	const previous = current;
	current = services;
	return () => {
		current = previous;
	};
}

/** Context is only readable during component init; everywhere else (event
 *  handlers, pure helpers, the export queue) falls back to the app-scoped value. */
function fromContext(): EditorServices | null {
	try {
		return hasContext(KEY) ? getContext<EditorServices>(KEY) : null;
	} catch {
		return null;
	}
}

export function getEditorServices(): EditorServices {
	const services = fromContext() ?? current;
	if (!services) throw new Error("EditorServices were never installed");
	return services;
}

/** For helpers that may run before a host installed anything: degrades instead
 *  of taking the render down. */
export function tryGetEditorServices(): EditorServices | null {
	return fromContext() ?? current;
}
