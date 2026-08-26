<script lang="ts">
import {
	ArrowRight,
	AtSign,
	BadgeCheck,
	Check,
	Clock,
	Copy,
	Download,
	ExternalLink,
	Eye,
	FileText,
	Film,
	Globe,
	LayoutDashboard,
	Link2,
	Lock,
	LogOut,
	Mail,
	Megaphone,
	MessageSquare,
	Moon,
	PencilLine,
	Play,
	RotateCcw,
	Search,
	Settings,
	Share2,
	ShieldOff,
	Sun,
	Trash2,
	User,
	UserCheck,
	Users,
	X,
} from "@recast/icons";
import {
	RecastPlayer,
	type RecastPlayerActionEvent,
	type RecastPlayerApi,
	type RecastPlayerTrack,
} from "@recast/player";
import { Button, buttonVariants } from "@recast/ui/button";
import * as Dialog from "@recast/ui/dialog";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import { toast } from "@recast/ui/sonner";
import { mode as themeMode, toggleMode } from "@recast/ui/theme";
import * as Tooltip from "@recast/ui/tooltip";
import { cn } from "@recast/ui/utils";
import { onMount, tick, untrack } from "svelte";
import { cubicOut, quintOut } from "svelte/easing";
import { Tween } from "svelte/motion";
import { fade, fly, scale, slide } from "svelte/transition";
import { browser } from "$app/environment";
import { goto, invalidateAll } from "$app/navigation";
import { page } from "$app/state";
import { analytics } from "$lib/analytics/client";
import { authClient } from "$lib/auth/client";
import { SeoMeta } from "$lib/components";
import Logo from "$lib/logo.svelte";
import {
	deleteComment,
	loadEngagement,
	postComment,
	type ReactionCount,
	readApiError,
	rememberViewerName,
	type ShareComment,
	shareSessionId,
	storedViewerName,
	toggleReaction,
} from "$lib/share/client";
import { toggleReactionState } from "$lib/share/engagement";
import {
	commentHue,
	compactTime,
	formatTime,
	initials,
	parseCommentText,
	parseTimeParam,
} from "$lib/share/format";
import ReactionIcon from "$lib/share/ReactionIcon.svelte";
import { REACTIONS } from "$lib/share/reactions";
import {
	activeCueIndex,
	filterCues,
	readCuesFromTrack,
	type TranscriptCue,
} from "$lib/share/transcript";
import {
	buildCommentMarkers,
	type LegacyVisibility,
	toLegacyVisibility,
	withTimeParam,
} from "./share-page.logic";

let { data } = $props();

// `access` is the server-resolved permission envelope. When `ok` is
// true it carries the recast + share + canManage; when false it
// carries the reason + (for same-team viewers) the owner contact so
// the denial card can render a "request access" affordance.
const access = $derived(data.access);
const okAccess = $derived(access.ok ? access : null);
const deniedAccess = $derived(access.ok ? null : access);
const recast = $derived(okAccess?.recast);

// Caption track for the player, when this recast has a captions sidecar.
const captionTracks = $derived<RecastPlayerTrack[]>(
	recast?.captions
		? [
				{
					src: recast.captions,
					kind: "captions",
					label: "English",
					srclang: "en",
					default: true,
				},
			]
		: [],
);

// ── Account-less invitee claim (selected shares) ──────────────────────
// A denied viewer of a `selected` share can request an email access link.
let claimEmail = $state("");
let claimState = $state<"idle" | "sending" | "sent">("idle");
// The verify endpoint bounces back with ?claim=invalid on a bad/expired link.
const claimInvalid = $derived(page.url.searchParams.get("claim") === "invalid");

async function submitClaim(e: SubmitEvent) {
	e.preventDefault();
	const email = claimEmail.trim();
	if (!email || claimState === "sending") return;
	claimState = "sending";
	try {
		const res = await fetch(`/api/share/${page.params.id}/claim`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ email }),
		});
		if (!res.ok) throw new Error(await readApiError(res, "Couldn't send the link."));
		claimState = "sent";
	} catch (err) {
		claimState = "idle";
		toast.error((err as Error)?.message ?? "Couldn't send the link.");
	}
}
const shareMeta = $derived(okAccess?.share);
const canManage = $derived(okAccess?.canManage ?? false);
// Exact source aspect ratio, when the recast row carries its dimensions.
// Passing this to the player reserves the precise box before metadata loads
// → zero layout shift. Null on legacy rows, where the player falls back to
// its 16/9 placeholder and adjusts once the real dimensions decode.
const playerAspect = $derived(
	recast?.width && recast?.height ? `${recast.width} / ${recast.height}` : null,
);
// Numeric aspect for theater sizing — the video is capped by available
// height (`max-width = height × ratio`) so it grows as large as the viewport
// allows without overflowing the fold. Falls back to 16:9 on legacy rows.
const heroRatio = $derived(recast?.width && recast?.height ? recast.width / recast.height : 16 / 9);
const slug = $derived(shareMeta?.slug ?? recast?.id ?? "");
const isDemo = $derived(slug === "demo");

// ── Social preview (dynamic OG) ───────────────────────────────────
// Shared recasts unfurl with a branded card that surfaces the recast's
// own title + description and who shared it, generated by /api/og so the
// Recast wordmark/footer branding stays consistent with the marketing
// pages. Denied/private shares fall back to a generic branded card. The
// page stays `noindex` either way (see <svelte:head> below) — rich link
// previews without making private content crawlable.
const ogTitle = $derived(recast?.title ?? "Private recast");
const ogDescription = $derived(
	recast?.description?.trim() ||
		(recast ? "Recorded, polished, and shared with Recast." : "This recast is private."),
);
const ogEyebrow = $derived(recast ? `Shared by ${recast.sharedBy}` : undefined);

// Record a real share view, keyed to the anonymous shareView session id so
// PostHog reconciles with the first-party watch-metrics table. Viewers are
// never identified — this stays anonymous regardless of consent.
onMount(() => {
	if (!browser || !okAccess || isDemo) return;
	analytics.capture("share_viewed", {
		visibility: shareMeta?.visibility,
		share_session_id: shareSessionId(),
	});
});

// `requiresPassword` is set by the page loader when the share has a
// passwordHash and the viewer doesn't carry a valid unlock cookie. In
// that case `recast.src` is empty and we render the prompt below
// instead of the player. On a successful unlock we `invalidateAll()`
// so the loader re-runs, signs the URL, and the player mounts.
const requiresPassword = $derived(
	Boolean(okAccess && "requiresPassword" in access && access.requiresPassword),
);
let passwordInput = $state("");
let unlocking = $state(false);
let unlockError = $state<string | null>(null);
async function submitUnlock(e: SubmitEvent) {
	e.preventDefault();
	if (!shareMeta || unlocking || !passwordInput) return;
	unlocking = true;
	unlockError = null;
	try {
		const res = await fetch(`/api/share/${shareMeta.slug}/unlock`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ password: passwordInput }),
		});
		if (!res.ok) {
			unlockError = res.status === 401 ? "Wrong password. Try again." : "Couldn't unlock.";
			return;
		}
		passwordInput = "";
		await invalidateAll();
	} catch {
		unlockError = "Network error. Try again.";
	} finally {
		unlocking = false;
	}
}

// The toggle below writes the standard {public, team, private} scopes
// (`workspace` shows as "team", its alias). The richer `selected`
// specific-people allowlist is created from the share dialog, not here —
// but we surface it ACCURATELY (instead of masking it as "Only me") so the
// owner sees the real scope and can switch off it.
// The share's true current scope (full enum), synced from the server and
// updated optimistically on toggle.
let currentScope = $state<string>(
	untrack(() => (data.access.ok ? data.access.share.visibility : "public")),
);
$effect(() => {
	if (access.ok) currentScope = access.share.visibility;
});
const isSelectedScope = $derived(currentScope === "selected");
// Which toggle row reads as active — none while on the specific-people
// allowlist, so "selected" no longer masquerades as "Only me".
const activeScope: LegacyVisibility | null = $derived(
	isSelectedScope ? null : toLegacyVisibility(currentScope),
);
let updatingVisibility = $state(false);

async function updateVisibility(next: "public" | "team" | "private") {
	if (!shareMeta || !canManage || updatingVisibility) return;
	if (next === activeScope) return;
	const previous = currentScope;
	currentScope = next;
	updatingVisibility = true;
	try {
		await toast.promise(
			(async () => {
				const res = await fetch(`/api/share/${shareMeta.slug}/access`, {
					method: "PATCH",
					headers: { "content-type": "application/json" },
					body: JSON.stringify({ visibility: next }),
				});
				if (!res.ok) {
					throw new Error(await readApiError(res, "Couldn't update access"));
				}
			})(),
			{
				loading: "Updating who can view…",
				success:
					next === "public"
						? "Anyone with the link can view."
						: next === "team"
							? "Restricted to your team."
							: "Only you can view.",
				error: (err) => (err as Error)?.message ?? "Couldn't update access.",
			},
		);
	} catch {
		currentScope = previous;
	} finally {
		updatingVisibility = false;
	}
}

// ── Player wiring ────────────────────────────────────────────────

const initialSeekSeconds = untrack(() => parseTimeParam(page.url.searchParams.get("t")));

let api = $state<RecastPlayerApi | null>(null);

// Re-seek every time the player API publishes — covers the initial
// `?t=` URL seed and any later remount. Reading `currentTime` via
// `untrack` so this effect only re-runs when `api` itself changes.
$effect(() => {
	if (!api) return;
	const target = untrack(() => Math.max(initialSeekSeconds, currentTime));
	if (target > 0) api.seek(target);
});

let currentTime = $state(initialSeekSeconds);
const smoothedTime = new Tween(0, { duration: 120, easing: cubicOut });
let watchedPct = $state(0);
let isPlaying = $state(false);
// True once the video reaches the end — drives the CTA end-card overlay.
let ended = $state(false);
// Pre-play hook: a sender + duration card owns the poster until the viewer
// commits to the first play. It's the highest-trust moment on the page (who
// sent this, how long), so we surface identity + duration instead of a bare
// poster. Cleared on the first view-start/progress or when the viewer plays here.
let hasStarted = $state(false);
function startPlayback() {
	hasStarted = true;
	api?.play();
}

$effect(() => {
	smoothedTime.set(currentTime);
});

// First-party view recording. Bumps the cached view count AND — critically
// — refreshes `recast.lastViewedAt`, which the Free-tier expiry sweep keys
// off (without this, watched recasts still archive at 14 days). Anonymous:
// keyed only by the share session fingerprint. Skipped for the demo (no
// real share row) and SSR. `keepalive` lets the "ended" beacon survive a
// tab close.
let viewStartSent = false;
function recordView(event: "start" | "ended") {
	if (!browser || isDemo || !slug) return;
	if (event === "start") {
		if (viewStartSent) return;
		viewStartSent = true;
	}
	try {
		void fetch(`/api/share/${slug}/view`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				sessionId: shareSessionId(),
				event,
				watchPct: Math.round(watchedPct),
				// Acquisition source — only meaningful on the first ("start")
				// beacon; the server reduces it to a bare host and drops self-refs.
				referrer: event === "start" ? document.referrer || null : null,
			}),
			keepalive: true,
		}).catch(() => {});
	} catch {
		// Never let a metrics beacon disrupt playback.
	}
}

function onEngagement(e: {
	type: "view-start" | "progress" | "ended";
	percent?: number;
	currentTime?: number;
}) {
	if (e.type === "view-start") {
		isPlaying = true;
		ended = false;
		hasStarted = true;
		recordView("start");
		trackPlayOnce();
	}
	if (e.type === "progress") {
		currentTime = e.currentTime ?? currentTime;
		watchedPct = e.percent ?? watchedPct;
		isPlaying = true;
		ended = false;
		hasStarted = true;
		// Some players jump straight to progress without a view-start; the
		// guard makes this idempotent so we still record the view once.
		recordView("start");
		trackPlayOnce();
		trackDepth(watchedPct);
	}
	if (e.type === "ended") {
		watchedPct = 100;
		isPlaying = false;
		ended = true;
		recordView("ended");
		trackDepth(100);
	}
}

function replay() {
	ended = false;
	api?.seek(0);
	api?.play();
}

// ── Call-to-action (owner-defined "next step") ───────────────────
let ctaLabel = $state(untrack(() => (data.access.ok ? data.access.share.ctaLabel : null)));
let ctaUrl = $state(untrack(() => (data.access.ok ? data.access.share.ctaUrl : null)));
const cta = $derived(ctaLabel && ctaUrl ? { label: ctaLabel, url: ctaUrl } : null);
// The owner's next-step card fires when they set a CTA. When they didn't,
// a finished stranger (not the owner) is the highest-intent moment to plant
// the growth loop — offer "record your own" instead of a blank overlay.
const showOwnerEndCard = $derived(ended && cta != null);
const showStrangerEndCard = $derived(ended && cta == null && !canManage);

// ── Comments + reactions (engagement layer) ──────────────────────
let commentsEnabled = $state(
	untrack(() => (data.access.ok ? data.access.share.commentsEnabled : true)),
);
// Owner opt-in to search indexing (public shares only). Optimistic, like the
// comments toggle; the <svelte:head> robots/canonical below key off it.
let searchable = $state(untrack(() => (data.access.ok ? data.access.share.searchable : false)));
const viewsCount = $derived(shareMeta?.viewsCount ?? 0);

let comments = $state<ShareComment[]>([]);
let reactions = $state<ReactionCount[]>([]);
let myReactions = $state<Set<string>>(new Set());
// Fire-once guard so the load effect doesn't re-trigger; distinct from the
// UI-facing `engagementState`, which drives skeleton / retry / empty.
let engagementLoaded = $state(false);
let engagementState = $state<"loading" | "ready" | "error">("loading");

// ── Engagement side-panel (Transcript | Comments) ─────────────────
// The video and the conversation live side-by-side so a viewer can read,
// skim the transcript, and jump moments without scrolling the player away.
const hasTranscript = $derived(captionTracks.length > 0);
type PanelTab = "transcript" | "comments";
let activeTab = $state<PanelTab>("comments");
// Skim-first: default to the transcript on captioned recasts, comments
// otherwise. Runs once, then a viewer's tab choice sticks.
let tabInitialized = false;
$effect(() => {
	if (tabInitialized || !recast) return;
	activeTab = hasTranscript ? "transcript" : "comments";
	tabInitialized = true;
});
// Keep the active tab valid if the owner toggles comments off mid-view.
$effect(() => {
	if (activeTab === "comments" && !commentsEnabled && hasTranscript) activeTab = "transcript";
	else if (activeTab === "transcript" && !hasTranscript && commentsEnabled) activeTab = "comments";
});

// Interactive transcript, read straight off the player's live caption
// `<track>` (no second fetch of the signed VTT, no CORS surface).
let transcriptCues = $state<TranscriptCue[]>([]);
let transcriptQuery = $state("");
let transcriptListEl = $state<HTMLElement | null>(null);
const filteredCues = $derived(filterCues(transcriptCues, transcriptQuery));
// The line to light as "now" — suppressed while searching (the filtered
// list no longer aligns with the playhead index).
const activeCue = $derived(
	transcriptQuery.trim() ? -1 : activeCueIndex(transcriptCues, smoothedTime.current),
);

$effect(() => {
	if (!browser || !api || !hasTranscript) return;
	const video = api.getVideoElement();
	if (!video) return;
	// Both the `<track>` registration and its cue parsing land
	// asynchronously after the player mounts, so poll for both.
	const attempt = () => {
		const track = Array.from(video.textTracks).find(
			(t) => t.kind === "captions" || t.kind === "subtitles",
		);
		if (!track) return false;
		// Populate cues without forcing captions on-screen — only nudge a
		// disabled track to `hidden`; never override the viewer's CC choice.
		if (track.mode === "disabled") track.mode = "hidden";
		const cues = readCuesFromTrack(track);
		if (cues.length) transcriptCues = cues;
		return cues.length > 0;
	};
	if (attempt()) return;
	let tries = 0;
	const iv = setInterval(() => {
		if (attempt() || ++tries > 25) clearInterval(iv);
	}, 200);
	return () => clearInterval(iv);
});

// Keep the active transcript line in view while it plays (not while the
// viewer is searching or reading a different tab).
$effect(() => {
	const idx = activeCue;
	if (idx < 0 || activeTab !== "transcript" || transcriptQuery.trim()) return;
	transcriptListEl
		?.querySelector<HTMLElement>(`[data-cue="${idx}"]`)
		?.scrollIntoView({ block: "nearest" });
});

// Comments projected onto the scrubber — click a marker (handled inside the
// player) to seek; we also surface the comment in the panel.
const commentMarkers = $derived(buildCommentMarkers(comments, commentHue));

function onPlayerAction(e: RecastPlayerActionEvent) {
	if (e.type !== "marker-select") return;
	activeTab = "comments";
	panelOpen = true;
	ended = false;
	if (!browser) return;
	const id = e.marker.id;
	queueMicrotask(() => {
		document
			.getElementById(`comment-${id}`)
			?.scrollIntoView({ block: "center", behavior: "smooth" });
	});
}

// Scroll-aware top bar: once the title scrolls under the sticky header
// we fade the owner + title into the bar so the viewer keeps context
// while reading/commenting. Driven off the title element itself via an
// IntersectionObserver (the `-72px` top margin ≈ the header height).
let titleAnchorEl = $state<HTMLElement | null>(null);
let showBarTitle = $state(false);
$effect(() => {
	const el = titleAnchorEl;
	if (!browser || !el) return;
	const io = new IntersectionObserver(
		([entry]) => {
			showBarTitle = !entry.isIntersecting;
		},
		{ rootMargin: "-72px 0px 0px 0px", threshold: 0 },
	);
	io.observe(el);
	return () => io.disconnect();
});

let sessionId = $state("");
let viewerName = $state("");
let draftText = $state("");
let inputEl = $state<HTMLInputElement | null>(null);
// Name is remembered, so it collapses to a compact "commenting as" line once
// set; `editingName` reopens the field, keeping the comment input the focus.
let editingName = $state(false);
let nameInputEl = $state<HTMLInputElement | null>(null);

async function editName() {
	// Only guests choose a name; a signed-in viewer's identity is their account.
	if (identityLocked) return;
	editingName = true;
	await tick();
	nameInputEl?.focus();
}

function commitName() {
	editingName = false;
	// Never write an account name into the guest name store — the two
	// identities must not bleed across sessions on a shared browser.
	if (!identityLocked && viewerName.trim()) rememberViewerName(viewerName);
}

onMount(() => {
	sessionId = shareSessionId();
	viewerName = storedViewerName();
});

// Load engagement once the player surface is live (covers both the
// initial mount and the post-unlock case, where the loader re-runs but
// the component doesn't re-mount).
$effect(() => {
	if (!browser || requiresPassword) return;
	const s = slug;
	if (!s || untrack(() => engagementLoaded)) return;
	loadAll(s);
});

async function loadAll(s: string) {
	engagementLoaded = true; // guard against the effect re-firing mid-await
	engagementState = "loading";
	const sid = shareSessionId();
	sessionId = sid;
	try {
		const e = await loadEngagement(s, sid);
		comments = e.comments;
		reactions = e.reactions;
		myReactions = new Set(e.myReactions);
		commentsEnabled = e.commentsEnabled;
		engagementState = "ready";
	} catch {
		// Surface a retry affordance rather than a false "No comments yet".
		engagementState = "error";
	}
}

// Manual retry after a failed engagement load (network / server error).
function retryEngagement() {
	if (slug) loadAll(slug);
}

async function refresh() {
	if (isDemo) return;
	try {
		const e = await loadEngagement(slug, sessionId);
		comments = e.comments;
		reactions = e.reactions;
		myReactions = new Set(e.myReactions);
		commentsEnabled = e.commentsEnabled;
	} catch {
		// best-effort
	}
}

function countFor(emoji: string): number {
	return reactions.find((r) => r.emoji === emoji)?.count ?? 0;
}
// The engagement rail (Transcript | Comments) lives in the page's layout as
// a collapsible sticky sidebar — not a slide-in sheet. The reaction-bar
// buttons toggle it: opening to a tab, switching tabs while open, or
// collapsing when you click the tab that's already showing. It only mounts
// when there's a populated tab, so the video reclaims full width when it's
// collapsed or on a bare recast.
const hasSidebar = $derived(hasTranscript || commentsEnabled);
let panelOpen = $state(false);
let railEl = $state<HTMLElement | null>(null);
async function togglePanel(tab: PanelTab) {
	if (panelOpen && activeTab === tab) {
		panelOpen = false;
		return;
	}
	activeTab = tab;
	panelOpen = true;
	// On mobile the rail stacks below the fold; without bringing it into view
	// the toggle reads as "nothing happened". Desktop docks it beside the
	// video (in-view already), so only scroll on the stacked breakpoint.
	if (browser && window.matchMedia("(max-width: 1023px)").matches) {
		await tick();
		railEl?.scrollIntoView({ behavior: "smooth", block: "start" });
	}
}

async function react(emoji: string) {
	const nextState = toggleReactionState({ myReactions, reactions }, emoji);
	myReactions = nextState.myReactions;
	reactions = nextState.reactions;
	if (isDemo) return;
	try {
		await toggleReaction(slug, { sessionId, emoji, atSeconds: Math.floor(currentTime) });
	} catch {
		refresh();
	}
}

async function submitComment() {
	const text = draftText.trim();
	const name = viewerName.trim();
	if (!text || !name) return;
	if (!identityLocked) rememberViewerName(name);
	const at = Math.floor(currentTime);
	if (isDemo) {
		comments = [
			...comments,
			{
				id: `local-${comments.length}`,
				authorName: name,
				atSeconds: at,
				body: text,
				createdAt: 0,
				mine: true,
				verified: identityLocked,
			},
		];
		draftText = "";
		toast.success(`Posted at ${formatTime(at)}.`);
		return;
	}
	try {
		const c = await postComment(slug, { sessionId, authorName: name, atSeconds: at, body: text });
		comments = [...comments, c];
		draftText = "";
		toast.success(`Posted at ${formatTime(at)}.`);
	} catch (e) {
		toast.error((e as Error)?.message ?? "Couldn't post comment.");
	}
}

async function removeComment(id: string) {
	const prev = comments;
	comments = comments.filter((c) => c.id !== id);
	if (isDemo) return;
	try {
		await deleteComment(slug, id, sessionId);
	} catch (e) {
		comments = prev;
		toast.error((e as Error)?.message ?? "Couldn't delete comment.");
	}
}

function insertCurrentTimestamp() {
	const stamp = `[${formatTime(currentTime)}] `;
	const el = inputEl;
	if (!el) {
		draftText += stamp;
		return;
	}
	const start = el.selectionStart ?? draftText.length;
	const end = el.selectionEnd ?? draftText.length;
	draftText = draftText.slice(0, start) + stamp + draftText.slice(end);
	queueMicrotask(() => {
		el.focus();
		const pos = start + stamp.length;
		el.setSelectionRange(pos, pos);
	});
}

// ── Owner share settings (CTA + comments toggle) ─────────────────
async function patchSettings(body: Record<string, unknown>) {
	const res = await fetch(`/api/share/${slug}/settings`, {
		method: "PATCH",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(body),
	});
	if (!res.ok) {
		throw new Error(await readApiError(res, "Couldn't update share settings"));
	}
	return (await res.json()) as {
		ctaLabel?: string | null;
		ctaUrl?: string | null;
		commentsEnabled?: boolean;
		searchable?: boolean;
		title?: string;
		description?: string | null;
	};
}

async function toggleCommentsEnabled() {
	const next = !commentsEnabled;
	commentsEnabled = next;
	if (isDemo) return;
	try {
		await patchSettings({ commentsEnabled: next });
		toast.success(next ? "Comments are on." : "Comments are off.");
	} catch (e) {
		commentsEnabled = !next;
		toast.error((e as Error)?.message ?? "Couldn't update comments.");
	}
}

async function toggleSearchable() {
	const next = !searchable;
	searchable = next;
	if (isDemo) return;
	try {
		await patchSettings({ searchable: next });
		toast.success(next ? "Listed in search engines." : "Hidden from search engines.");
	} catch (e) {
		searchable = !next;
		toast.error((e as Error)?.message ?? "Couldn't update search visibility.");
	}
}

let ctaDialogOpen = $state(false);
let ctaLabelDraft = $state("");
let ctaUrlDraft = $state("");
let savingCta = $state(false);

function openCtaEditor() {
	ctaLabelDraft = ctaLabel ?? "";
	ctaUrlDraft = ctaUrl ?? "";
	ctaDialogOpen = true;
}

async function saveCta(e: SubmitEvent) {
	e.preventDefault();
	const label = ctaLabelDraft.trim();
	const url = ctaUrlDraft.trim();
	if (label && !url) {
		toast.error("Add a link for the button.");
		return;
	}
	savingCta = true;
	if (isDemo) {
		ctaLabel = label || null;
		ctaUrl = url || null;
		ctaDialogOpen = false;
		savingCta = false;
		return;
	}
	try {
		const r = await patchSettings({ ctaLabel: label, ctaUrl: url });
		ctaLabel = r.ctaLabel ?? null;
		ctaUrl = r.ctaUrl ?? null;
		ctaDialogOpen = false;
		toast.success(ctaLabel ? "Call-to-action saved." : "Call-to-action removed.");
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't save the call-to-action.");
	} finally {
		savingCta = false;
	}
}

function jumpTo(seconds: number) {
	api?.seek(seconds);
	if (!isPlaying) api?.play();
	ended = false;
	if (!browser) return;
	const href = withTimeParam(new URL(window.location.href), seconds);
	window.history.replaceState({}, "", href);
}

// ── Owner-editable video details (title + description — the recast's own
//    text, shown on the page and reused for the OG card). Locally mirrored
//    so an owner edit reflects at once; server-synced on navigation. ──────
let titleText = $state(untrack(() => (data.access.ok ? data.access.recast.title : "")));
let descriptionText = $state(untrack(() => (data.access.ok ? data.access.recast.description : "")));
$effect(() => {
	if (access.ok) {
		titleText = access.recast.title;
		descriptionText = access.recast.description;
	}
});
let detailsOpen = $state(false);
let titleDraft = $state("");
let descDraft = $state("");
let savingDetails = $state(false);

function openDetailsEditor() {
	titleDraft = titleText ?? "";
	descDraft = descriptionText ?? "";
	detailsOpen = true;
}

async function saveDetails(e: SubmitEvent) {
	e.preventDefault();
	const title = titleDraft.trim();
	const description = descDraft.trim();
	if (!title) {
		toast.error("Title can't be empty.");
		return;
	}
	savingDetails = true;
	if (isDemo) {
		titleText = title;
		descriptionText = description;
		detailsOpen = false;
		savingDetails = false;
		return;
	}
	try {
		const r = await patchSettings({ title, description });
		titleText = r.title ?? title;
		descriptionText = r.description ?? "";
		detailsOpen = false;
		toast.success("Details saved.");
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't save the details.");
	} finally {
		savingDetails = false;
	}
}

async function writeClipboard(text: string, okMsg: string) {
	try {
		await navigator.clipboard.writeText(text);
		toast.success(okMsg);
	} catch {
		toast.error("Couldn't copy to clipboard.");
	}
}

async function copyShareLink() {
	if (!browser) return;
	// Seconds 0 clears any `?t=` so the copied link starts from the top.
	await writeClipboard(withTimeParam(new URL(window.location.href), 0), "Share link copied.");
}

async function copyLinkAtCurrentTime() {
	if (!browser) return;
	const href = withTimeParam(new URL(window.location.href), currentTime);
	const t = compactTime(currentTime);
	await writeClipboard(href, t ? `Link copied at ${formatTime(currentTime)}.` : "Link copied.");
}

// ── Viewer identity ──────────────────────────────────────────────
type SessionShape = {
	data: {
		user?: { name?: string | null; email?: string | null; image?: string | null } | null;
	} | null;
};
const session = authClient.useSession();
const viewer = $derived(($session as unknown as SessionShape).data?.user ?? null);
// Signed-in viewers comment AS their account — the name is locked, not a
// guest field. Guests (no session) still self-supply and remember a name.
const identityLocked = $derived(viewer != null);
// Account avatar for the composer identity chip: real image when present,
// else initials on the neutral account swatch (distinct from guests' hued dots).
const viewerInitials = $derived(initials(viewer?.name, viewer?.email));

// Force the composer name to the account once the session hydrates — and keep
// it synced — so a signed-in viewer can't post under a stale guest name.
$effect(() => {
	if (viewer) viewerName = viewer.name?.trim() || viewer.email?.split("@")[0] || "Recast user";
});

async function signOut() {
	await authClient.signOut();
	await goto("/");
}

// Share→signup is the page's growth loop: a stranger watching a polished
// recast is the highest-intent moment to convert. Track each acquisition
// surface so the funnel is measurable by placement (header / end-card / mark).
function trackSignupCta(placement: "header" | "end-card" | "mid-watch" | "positioning-chip") {
	if (!browser) return;
	analytics.capture("share_signup_cta_click", {
		placement,
		visibility: shareMeta?.visibility,
	});
}

// ── Funnel instrumentation ────────────────────────────────────────
// share_viewed (mount) → share_play_started (real play) → share_watch_depth
// (25/50/75/100) → share_cta_impression / share_signup_cta_click. Together
// these give play-rate, drop-off, and per-placement CTA click-through, which
// clicks-only tracking couldn't. Skipped for the demo (no real share row).
let analyticsPlaySent = false;
function trackPlayOnce() {
	if (analyticsPlaySent || !browser || isDemo) return;
	analyticsPlaySent = true;
	analytics.capture("share_play_started", {
		visibility: shareMeta?.visibility,
		share_session_id: shareSessionId(),
	});
}
const depthSeen = new Set<number>();
function trackDepth(pct: number) {
	if (!browser || isDemo) return;
	for (const m of [25, 50, 75, 100]) {
		if (pct >= m && !depthSeen.has(m)) {
			depthSeen.add(m);
			analytics.capture("share_watch_depth", {
				pct: m,
				visibility: shareMeta?.visibility,
			});
		}
	}
}
function trackCtaImpression(placement: "end-card" | "mid-watch" | "positioning-chip") {
	if (!browser || isDemo) return;
	analytics.capture("share_cta_impression", {
		placement,
		visibility: shareMeta?.visibility,
	});
}

// ── Growth loop for the majority who never finish ─────────────────
// The end-card only converts finishers. This reaches the same audience — an
// anonymous stranger on a public share with no owner CTA of its own — while
// they watch (a one-time nudge at 50%) and persistently (a positioning chip
// under the title). Signed-in viewers and owners are excluded (nothing to
// convert), and an owner's own CTA always takes precedence over ours.
const strangerLoop = $derived(currentScope === "public" && !canManage && !viewer && !cta);

const nudgeKey = $derived(slug ? `recast:share-nudge:${slug}` : null);
let nudgeDismissed = $state(false);
let nudgeArmed = $state(false);
onMount(() => {
	if (browser && nudgeKey) nudgeDismissed = localStorage.getItem(nudgeKey) === "1";
});
// Never float over an immersive/fullscreen watch. (A fullscreen video also
// hides page-level fixed elements, but tracking it keeps the state honest and
// prevents any flash on exit.)
let isFullscreen = $state(false);
onMount(() => {
	if (!browser) return;
	const sync = () => (isFullscreen = !!document.fullscreenElement);
	document.addEventListener("fullscreenchange", sync);
	sync();
	return () => document.removeEventListener("fullscreenchange", sync);
});
const showMidWatchNudge = $derived(
	strangerLoop && nudgeArmed && !nudgeDismissed && !ended && !isFullscreen,
);
$effect(() => {
	if (strangerLoop && !nudgeArmed && !nudgeDismissed && !ended && watchedPct >= 50) {
		nudgeArmed = true;
		trackCtaImpression("mid-watch");
	}
});
function dismissNudge() {
	nudgeDismissed = true;
	if (browser && nudgeKey) {
		try {
			localStorage.setItem(nudgeKey, "1");
		} catch {
			// Private mode: a session-only dismissal is an acceptable fallback.
		}
	}
}

// End-card click-through needs its impression counted once per session.
let endCardImpressionSent = false;
$effect(() => {
	if (showStrangerEndCard && !endCardImpressionSent) {
		endCardImpressionSent = true;
		trackCtaImpression("end-card");
	}
});

// The persistent positioning chip is always-on for the stranger loop; count
// its impression once so its click-through is comparable to the others.
let chipImpressionSent = false;
$effect(() => {
	if (strangerLoop && !chipImpressionSent) {
		chipImpressionSent = true;
		trackCtaImpression("positioning-chip");
	}
});
</script>

<SeoMeta title={ogTitle} description={ogDescription} eyebrow={ogEyebrow} />

<svelte:head>
	<!-- Crawlable only when the share is public AND the owner opted in; everything
	     else stays noindex. `currentScope` and `searchable` are both reactive, so
	     flipping visibility or the toggle updates this live. Rich link previews
	     (og:) come from <SeoMeta> regardless. -->
	{#if currentScope === "public" && searchable}
		<meta name="robots" content="index,follow" />
		<link rel="canonical" href={page.url.origin + page.url.pathname} />
	{:else}
		<meta name="robots" content="noindex" />
	{/if}
</svelte:head>

{#if deniedAccess}
	<!-- Denial fallback. Same-team viewers get a "request access" CTA that
	     opens their mail client to the owner; strangers get a plain card.
	     Signed-out viewers see a sign-in nudge. -->
	<div class="relative grid min-h-screen place-items-center px-6 py-16 text-foreground">
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-0 -z-10"
			style="background: radial-gradient(ellipse 60% 40% at 50% 0%, color-mix(in srgb, var(--color-primary) 6%, transparent), transparent 70%);"
		></div>
		<div
			aria-hidden="true"
			class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 -z-10 opacity-25"
		></div>

		<div
			class="glass-card w-full max-w-md rounded-2xl border border-border-low/40 p-7 shadow-craft-xl"
			in:scale={{ start: 0.96, duration: 320, easing: quintOut, opacity: 0.6 }}
		>
			<div class="flex items-start gap-3">
				<span class="grid size-10 shrink-0 place-items-center rounded-xl bg-foreground/5 text-muted-foreground ring-1 ring-border/40">
					<ShieldOff class="size-5" />
				</span>
				<div class="min-w-0">
					<h1 class="text-lg font-semibold tracking-tight">You don't have access</h1>
					<p class="mt-1 text-sm text-muted-foreground">
						{#if deniedAccess.visibility === "selected"}
							This recording is shared with specific people. Enter your email to get a one-time access link. No account needed.
						{:else if deniedAccess.visibility === "team"}
							This recast is shared with a specific team. Ask the owner to add you, or sign in with an account that's a member.
						{:else if deniedAccess.visibility === "private"}
							This recast is private. Only the owner can view it.
						{:else}
							This share link isn't available to your account.
						{/if}
					</p>
				</div>
			</div>

			{#if deniedAccess.visibility === "selected"}
				<!-- Invite-only: email-based access claim. -->
				{#if claimState === "sent"}
					<div class="mt-5 rounded-xl border border-primary/30 bg-primary/8 p-4 text-sm" in:fly={{ y: 8, duration: 240, easing: cubicOut }}>
						<p class="flex items-center gap-2 font-medium text-foreground">
							<Check class="size-4 text-primary" />
							Check your inbox
						</p>
						<p class="mt-1 text-muted-foreground">
							If <span class="font-medium text-foreground">{claimEmail}</span> is on the access list, we've sent a link to open this recording.
						</p>
					</div>
				{:else}
					<form class="mt-5 flex flex-col gap-2" onsubmit={submitClaim}>
						{#if claimInvalid}
							<p class="text-xs text-destructive" in:slide={{ duration: 160 }}>
								That access link is invalid or has expired. Enter your email to get a fresh one.
							</p>
						{/if}
						<div class="flex flex-col gap-2 sm:flex-row">
							<Input
								type="email"
								bind:value={claimEmail}
								placeholder="you@company.com"
								autocomplete="email"
								required
								class="flex-1"
							/>
							<Button type="submit" class="gap-2" disabled={claimState === "sending"}>
								<Mail class="size-3.5" />
								{claimState === "sending" ? "Sending…" : "Email me a link"}
							</Button>
						</div>
					</form>
					<div class="mt-2">
						<Button href="/dashboard" variant="outline" class="w-full gap-2">
							<LayoutDashboard class="size-3.5" />
							Back to dashboard
						</Button>
					</div>
				{/if}
			{:else}
			<div class="mt-5 flex flex-col gap-2">
				{#if deniedAccess.sameTeam && deniedAccess.ownerEmail}
					<Button
						href={`mailto:${deniedAccess.ownerEmail}?subject=${encodeURIComponent("Recast access request")}&body=${encodeURIComponent("Hi — could you share access to this recast with me? " + (browser ? window.location.href : ""))}`}
						class="gap-2"
					>
						<Mail class="size-3.5" />
						Request access from {deniedAccess.ownerEmail}
					</Button>
				{:else if viewer == null}
					<Button href={`/login?next=${encodeURIComponent(browser ? window.location.pathname + window.location.search : "")}`} class="gap-2">
						<User class="size-3.5" />
						Sign in to check access
					</Button>
				{/if}
				<Button href="/dashboard" variant="outline" class="gap-2">
					<LayoutDashboard class="size-3.5" />
					Back to dashboard
				</Button>
			</div>
			{/if}
		</div>
	</div>
{:else if requiresPassword}
	<!-- Password-protected share. Same chrome as the denial card. -->
	<div class="relative grid min-h-screen place-items-center px-6 py-16 text-foreground">
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-0 -z-10"
			style="background: radial-gradient(ellipse 60% 40% at 50% 0%, color-mix(in srgb, var(--color-primary) 6%, transparent), transparent 70%);"
		></div>
		<div
			aria-hidden="true"
			class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 -z-10 opacity-25"
		></div>

		<form
			class="glass-card w-full max-w-md rounded-2xl border border-border-low/40 p-7 shadow-craft-xl"
			in:scale={{ start: 0.96, duration: 320, easing: quintOut, opacity: 0.6 }}
			onsubmit={submitUnlock}
		>
			<div class="flex items-start gap-3">
				<span class="grid size-10 shrink-0 place-items-center rounded-xl bg-foreground/5 text-foreground ring-1 ring-border/40">
					<Lock class="size-5" />
				</span>
				<div class="min-w-0">
					<h1 class="text-lg font-semibold tracking-tight">Password required</h1>
					<p class="mt-1 text-sm text-muted-foreground">
						This recast is password-protected. Enter the password the owner shared with you.
					</p>
				</div>
			</div>

			<label class="mt-5 block">
				<span class="sr-only">Password</span>
				<input
					type="password"
					required
					autocomplete="current-password"
					bind:value={passwordInput}
					class="w-full rounded-lg border border-border-low/70 bg-background/80 px-3.5 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
					placeholder="Password"
					disabled={unlocking}
				/>
			</label>

			{#if unlockError}
				<p class="mt-2 text-xs text-destructive">{unlockError}</p>
			{/if}

			<div class="mt-4 flex flex-col gap-2">
				<Button type="submit" disabled={unlocking || !passwordInput} class="gap-2">
					{unlocking ? "Unlocking…" : "Unlock"}
					{#if !unlocking}<ArrowRight class="size-3.5" />{/if}
				</Button>
			</div>
		</form>
	</div>
{:else}
	<!-- ── Standard share view ─────────────────────────────────────────
	     Single, viewer-first layout: quiet top bar → player hero → title
	     + meta + CTA → reactions + comments. No "view modes": a stranger
	     should be able to watch with zero learning curve. -->
	<div
		class="relative min-h-screen text-foreground"
		in:fade={{ duration: 360, easing: quintOut }}
	>
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-0 -z-10"
			style="background: radial-gradient(ellipse 70% 50% at 50% 0%, color-mix(in srgb, var(--color-primary) 9%, transparent), transparent 72%);"
		></div>
		<div
			aria-hidden="true"
			class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 -z-10 opacity-30"
		></div>

		<!-- Top bar — brand left, light viewer/owner actions right. The mode
		     switcher is gone; the only chrome here is theme, share, account. -->
		<header class="sticky top-0 z-30 border-b border-border-low/30 bg-background/70 backdrop-blur-xl">
			<div class="relative mx-auto flex w-full max-w-400 items-center gap-3 px-5 py-3 sm:px-6 lg:px-8">
				<!-- Left mark. For Pro shares this should swap to the owner's
				     custom logo (branding feature, not wired yet) — the slot is
				     here so that change is a one-line conditional later. -->
				<a href="/" class="group/logo flex shrink-0 items-center gap-2.5" aria-label="Recast — home">
					<span class="grid size-9 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]">
						<Logo size="22" color="transparent" fill="currentColor" />
					</span>
					<span class="text-base font-semibold tracking-tight text-foreground max-sm:hidden">Recast</span>
				</a>

				<!-- Scroll-aware context — absolutely centered so it never
				     shifts the logo/actions, pointer-events-none so it's purely
				     informational. Fades in once the title scrolls away. -->
				{#if showBarTitle && recast}
					<div
						class="pointer-events-none absolute left-1/2 flex max-w-[52%] -translate-x-1/2 items-center gap-2"
						in:fly={{ y: -6, duration: 200, easing: cubicOut }}
						out:fade={{ duration: 140 }}
					>
						<span class="grid size-6 shrink-0 place-items-center rounded-full bg-foreground/10 text-[9px] font-bold text-foreground ring-1 ring-border/40">
							{initials(recast.sharedBy, null)}
						</span>
						<span class="truncate text-sm font-medium text-foreground/90">{titleText}</span>
					</div>
				{/if}

				<div class="ml-auto flex shrink-0 items-center gap-2">
					<!-- Theme toggle -->
					<Tooltip.Provider delayDuration={300}>
						<Tooltip.Root>
							<Tooltip.Trigger
								onclick={toggleMode}
								aria-label={themeMode.current === "dark" ? "Switch to light mode" : "Switch to dark mode"}
								class={cn(buttonVariants({ variant: "ghost", size: "icon-sm" }), "text-muted-foreground")}
							>
								<span class="relative grid size-3.5 place-items-center">
									{#if themeMode.current === "dark"}
										<span class="absolute" in:fly={{ y: 4, duration: 180, easing: cubicOut }} out:fade={{ duration: 120 }}>
											<Sun class="size-3.5" />
										</span>
									{:else}
										<span class="absolute" in:fly={{ y: -4, duration: 180, easing: cubicOut }} out:fade={{ duration: 120 }}>
											<Moon class="size-3.5" />
										</span>
									{/if}
								</span>
							</Tooltip.Trigger>
							<Tooltip.Content sideOffset={8}>
								<span class="text-[11px]">{themeMode.current === "dark" ? "Light mode" : "Dark mode"}</span>
							</Tooltip.Content>
						</Tooltip.Root>
					</Tooltip.Provider>

					<!-- Share menu — viewer actions on top; owner controls (who
					     can view, comments, CTA, analytics) gated behind canManage. -->
					<DropdownMenu.Root>
						<DropdownMenu.Trigger
							class={buttonVariants({ variant: canManage ? "default" : "outline", size: "sm" })}
						>
							<Share2 class="size-3.5" />
							<span class="max-sm:hidden">Share</span>
						</DropdownMenu.Trigger>
						<DropdownMenu.Content align="end" sideOffset={6} class="w-64">
							<DropdownMenu.Label class="text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
								Share this recast
							</DropdownMenu.Label>
							<DropdownMenu.Item onclick={copyShareLink} class="gap-2.5">
								<Copy class="size-3.5 text-muted-foreground" />
								Copy link
							</DropdownMenu.Item>
							<DropdownMenu.Item onclick={copyLinkAtCurrentTime} class="gap-2.5">
								<Link2 class="size-3.5 text-muted-foreground" />
								Copy link at
								<span class="ml-auto font-mono text-[10px] tabular-nums text-foreground">{formatTime(currentTime)}</span>
							</DropdownMenu.Item>
							{#if canManage}
								<!-- Downloading the original master is an owner/admin
								     action only — viewers stream but can't pull the file. -->
								<DropdownMenu.Separator />
								<DropdownMenu.Item onclick={() => browser && recast && window.open(recast.src, "_blank")} class="gap-2.5">
									<Download class="size-3.5 text-muted-foreground" />
									Download original
								</DropdownMenu.Item>
								<DropdownMenu.Separator />
								<DropdownMenu.Label class="text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
									Who can view
								</DropdownMenu.Label>
								{#if isSelectedScope}
									<!-- Current scope is the specific-people allowlist. It's set
									     up from the share dialog (not editable inline), so we show
									     it as the active state and let the owner switch to a
									     standard scope below. -->
									<div class="flex items-center gap-2.5 rounded-md px-2 py-1.5">
										<UserCheck class="size-3.5 text-primary" />
										<div class="flex-1 min-w-0">
											<div class="text-xs font-medium text-foreground">Specific people</div>
											<div class="text-[10px] text-muted-foreground">Only invited people can view</div>
										</div>
										<Check class="size-3.5 text-primary" />
									</div>
								{/if}
								{#each [
									{ id: "public" as const, label: "Anyone with the link", icon: Globe },
									{ id: "team" as const, label: "Only my team", icon: Users },
									{ id: "private" as const, label: "Only me", icon: Lock },
								] as opt (opt.id)}
									{@const active = activeScope === opt.id}
									<DropdownMenu.Item
										disabled={updatingVisibility}
										onSelect={(e) => {
											e.preventDefault();
											updateVisibility(opt.id);
										}}
										class="gap-2.5"
									>
										<opt.icon class={cn("size-3.5", active ? "text-primary" : "text-muted-foreground")} />
										<div class="flex-1 min-w-0">
											<div class={cn("text-xs", active && "font-medium text-foreground")}>{opt.label}</div>
										</div>
										{#if active}<Check class="size-3.5 text-primary" />{/if}
									</DropdownMenu.Item>
								{/each}

								<DropdownMenu.Separator />
								<DropdownMenu.Label class="text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
									Engagement
								</DropdownMenu.Label>
								<DropdownMenu.Item onclick={openCtaEditor} class="gap-2.5">
									<Megaphone class="size-3.5 text-muted-foreground" />
									{cta ? "Edit call-to-action" : "Add a call-to-action"}
								</DropdownMenu.Item>
								<DropdownMenu.Item
									onSelect={(e) => {
										e.preventDefault();
										toggleCommentsEnabled();
									}}
									class="gap-2.5"
								>
									<MessageSquare class="size-3.5 text-muted-foreground" />
									<div class="flex-1 min-w-0"><div class="text-xs">Comments</div></div>
									<span class={cn("font-mono text-[10px] uppercase", commentsEnabled ? "text-primary" : "text-muted-foreground")}>
										{commentsEnabled ? "On" : "Off"}
									</span>
								</DropdownMenu.Item>
								{#if currentScope === "public"}
									<!-- Search indexing only matters for a public link, so the
									     toggle only appears there. Default off; opting in emits
									     index,follow + a canonical (see <svelte:head>). -->
									<DropdownMenu.Item
										onSelect={(e) => {
											e.preventDefault();
											toggleSearchable();
										}}
										class="gap-2.5"
									>
										<Search class="size-3.5 text-muted-foreground" />
										<div class="flex-1 min-w-0">
											<div class="text-xs">Search engines</div>
											<div class="text-[10px] text-muted-foreground">Let this show up in search results</div>
										</div>
										<span class={cn("font-mono text-[10px] uppercase", searchable ? "text-primary" : "text-muted-foreground")}>
											{searchable ? "On" : "Off"}
										</span>
									</DropdownMenu.Item>
								{/if}
								<DropdownMenu.Item onclick={() => goto("/dashboard/analytics")} class="gap-2.5">
									<Eye class="size-3.5 text-muted-foreground" />
									View analytics
								</DropdownMenu.Item>
							{/if}
						</DropdownMenu.Content>
					</DropdownMenu.Root>

					{#if viewer}
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								aria-label="Account menu — {viewer.name || viewer.email}"
								class="grid size-8 shrink-0 place-items-center overflow-hidden rounded-full bg-foreground text-[11px] font-bold text-background ring-1 ring-border/40 transition-transform hover:scale-105 focus-visible:outline-2 focus-visible:outline-primary"
							>
								{#if viewer.image}
									<img src={viewer.image} alt="" referrerpolicy="no-referrer" class="size-full object-cover" />
								{:else}
									{initials(viewer.name, viewer.email)}
								{/if}
							</DropdownMenu.Trigger>
							<DropdownMenu.Content align="end" sideOffset={8} class="w-64">
								<div class="flex items-center gap-2.5 px-2 py-2.5">
									<span class="grid size-9 shrink-0 place-items-center overflow-hidden rounded-full bg-foreground text-[12px] font-bold text-background ring-1 ring-border/40">
										{#if viewer.image}
											<img src={viewer.image} alt="" referrerpolicy="no-referrer" class="size-full object-cover" />
										{:else}
											{initials(viewer.name, viewer.email)}
										{/if}
									</span>
									<div class="min-w-0 flex-1">
										<div class="truncate text-sm font-semibold text-foreground">{viewer.name || "Recast user"}</div>
										<div class="truncate font-mono text-[10px] text-muted-foreground">{viewer.email}</div>
									</div>
								</div>
								<DropdownMenu.Separator />
								<DropdownMenu.Item onclick={() => goto("/dashboard")} class="gap-2.5">
									<LayoutDashboard class="size-3.5 text-muted-foreground" />
									Dashboard
								</DropdownMenu.Item>
								<DropdownMenu.Item onclick={() => goto("/dashboard/recasts")} class="gap-2.5">
									<Film class="size-3.5 text-muted-foreground" />
									My recasts
								</DropdownMenu.Item>
								<DropdownMenu.Item onclick={() => goto("/dashboard/settings/profile")} class="gap-2.5">
									<Settings class="size-3.5 text-muted-foreground" />
									Account settings
								</DropdownMenu.Item>
								<DropdownMenu.Separator />
								<DropdownMenu.Item onclick={signOut} class="gap-2.5 text-destructive focus:bg-destructive/10 focus:text-destructive">
									<LogOut class="size-3.5" />
									Sign out
								</DropdownMenu.Item>
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{:else if currentScope === "public"}
						<!-- Public share = viral traffic. The video is the demo, so this
						     is the one persistent, honest path from watching to making
						     your own. Sign-in stays for returning users but is demoted;
						     the primary weight goes to acquisition. -->
						<Button href="/login" size="sm" variant="ghost" class="max-sm:hidden">
							Sign in
						</Button>
						<Button href="/signup" onclick={() => trackSignupCta("header")} size="sm">
							Try Recast free
						</Button>
					{:else}
						<!-- Access-gated link: signing in is what could unlock it, so it
						     leads (and carries the return path back to this share). -->
						<Button
							href={`/login?next=${encodeURIComponent(browser ? window.location.pathname + window.location.search : "")}`}
							size="sm"
							variant="outline"
							class="gap-1.5"
						>
							<User class="size-3.5" />
							<span class="max-sm:hidden">Sign in</span>
						</Button>
					{/if}
				</div>
			</div>
		</header>

		<main class="share-main relative mx-auto w-full max-w-400 px-4 pb-24 pt-6 sm:px-6 lg:px-8" data-has-rail={hasSidebar} data-rail={panelOpen && hasSidebar ? "open" : "closed"}>
			<!-- Video-first: the player IS the page. Theatmax-w-400 fills as
			     much of the viewport as the fold allows; the conversation and
			     transcript are on-demand chrome (floating bar → docked panel), so
			     watching a stranger's video needs zero learning curve.
			     End-card overlays the player when the video ends — the owner's
			     next-step CTA, or (for a stranger) a nudge to record their own. -->
			<section class="relative mx-auto w-full" style="max-width: min(100%, calc((100dvh - 15rem) * {heroRatio}));">
				<div class="glass-card relative overflow-hidden rounded-2xl shadow-craft-xl">
					{#if recast?.src}
						<!-- Isolate the player: an hls.js / media-chrome render or effect
						     error degrades to a recoverable fallback instead of white-
						     screening the whole share view. -->
						<svelte:boundary onerror={(err) => browser && analytics.capture("share_player_error", { reason: (err as Error)?.name ?? "unknown" })}>
							<RecastPlayer
								bind:api
								src={recast.src}
								poster={recast.poster}
								title={recast.title}
								aspectRatio={playerAspect}
								tracks={captionTracks}
								markers={commentMarkers}
								onengagement={onEngagement}
								onaction={onPlayerAction}
							/>
							{#snippet failed(_error, reset)}
								<div class="grid aspect-video place-items-center gap-3 bg-foreground/5 px-6 text-center">
									<p class="text-sm text-muted-foreground">Something went wrong loading the player.</p>
									<Button size="sm" variant="outline" onclick={reset} class="gap-1.5">
										<RotateCcw class="size-3.5" />
										Try again
									</Button>
								</div>
							{/snippet}
						</svelte:boundary>
					{:else}
						<div class="grid aspect-video place-items-center bg-foreground/5 text-sm text-muted-foreground">
							Playback is unavailable for this recast.
						</div>
					{/if}

					{#if recast?.src && !hasStarted && !ended}
						<!-- Pre-play hook. Owns the poster until the first play: sender
						     identity (trust) + duration (expectation) are the two things
						     that decide whether a stranger commits. A full-surface button
						     under pointer-events-none content makes the whole frame play,
						     while the visible play glyph tracks the group hover. z-40 sits
						     above media-chrome controls but below the z-50 end-cards. -->
						<div
							class="group/preplay absolute inset-0 z-40 grid place-items-center bg-gradient-to-b from-black/45 via-black/25 to-black/70 px-6 backdrop-blur-[2px]"
							out:fade={{ duration: 220, easing: cubicOut }}
						>
							<button
								type="button"
								onclick={startPlayback}
								aria-label="Play — {titleText}"
								class="absolute inset-0 cursor-pointer rounded-2xl focus-visible:outline-2 focus-visible:-outline-offset-4 focus-visible:outline-white"
							></button>
							<div class="pointer-events-none relative flex flex-col items-center gap-5 text-center">
								<div class="flex items-center gap-2.5">
									<span class="grid size-9 place-items-center rounded-full bg-white/15 text-xs font-bold text-white ring-1 ring-white/25 backdrop-blur">
										{initials(recast.sharedBy, null)}
									</span>
									<span class="text-sm text-white/85"><span class="font-semibold text-white">{recast.sharedBy}</span> shared a video</span>
								</div>

								<span class="grid size-16 place-items-center rounded-full bg-white/95 text-black shadow-craft-xl transition-transform duration-200 group-hover/preplay:scale-105 group-active/preplay:scale-95 sm:size-20">
									<Play class="size-7 translate-x-0.5 fill-current sm:size-8" />
								</span>

								<div class="flex max-w-lg flex-col items-center gap-2.5">
									<span class="text-balance text-lg font-semibold leading-tight text-white sm:text-xl">{titleText}</span>
									<div class="flex flex-wrap items-center justify-center gap-2 text-xs font-medium text-white/75">
										{#if recast.durationSec}
											<span class="inline-flex items-center gap-1 rounded-full bg-black/35 px-2.5 py-1 ring-1 ring-white/15">
												<Clock class="size-3" />
												<span class="font-mono tabular-nums">{formatTime(recast.durationSec)}</span>
											</span>
										{/if}
										{#if viewsCount > 0}
											<span class="inline-flex items-center gap-1 rounded-full bg-black/35 px-2.5 py-1 ring-1 ring-white/15">
												<Eye class="size-3" />
												{viewsCount.toLocaleString()} {viewsCount === 1 ? "view" : "views"}
											</span>
										{/if}
									</div>
								</div>
							</div>
						</div>
					{/if}

					{#if showOwnerEndCard && cta}
						<!-- z-50 lifts the end-card above the player's controls
						     (media-chrome tops out at z-index 6); without it the
						     center play button sits on top and eats the clicks
						     meant for the CTA / Replay. -->
						<div
							class="absolute inset-0 z-50 grid place-items-center bg-black/70 backdrop-blur-sm"
							in:fade={{ duration: 220, easing: cubicOut }}
						>
							<div class="flex flex-col items-center gap-4 px-6 text-center" in:scale={{ start: 0.95, duration: 280, easing: quintOut, opacity: 0.4 }}>
								<p class="text-sm font-medium text-white/70">Thanks for watching</p>
								<Button href={cta.url} target="_blank" rel="noopener" size="lg" class="gap-2">
									{cta.label}
									<ExternalLink class="size-4" />
								</Button>
								<button
									type="button"
									onclick={replay}
									class="inline-flex items-center gap-1.5 text-xs font-medium text-white/60 transition-colors hover:text-white"
								>
									<RotateCcw class="size-3.5" />
									Replay
								</button>
							</div>
						</div>
					{:else if showStrangerEndCard}
						<!-- No owner CTA: turn the finished-watching moment into the
						     growth loop instead of a blank overlay. -->
						<div
							class="absolute inset-0 z-50 grid place-items-center bg-black/70 backdrop-blur-sm"
							in:fade={{ duration: 220, easing: cubicOut }}
						>
							<div class="flex flex-col items-center gap-4 px-6 text-center" in:scale={{ start: 0.95, duration: 280, easing: quintOut, opacity: 0.4 }}>
								<span class="grid size-11 place-items-center rounded-2xl bg-white/10 p-1.5 text-white ring-1 ring-white/15">
									<Logo size="24" color="transparent" fill="currentColor" />
								</span>
								<p class="max-w-xs text-sm font-medium text-white/80">Record, polish, and share videos like this. Free with Recast.</p>
								<Button href="/signup" onclick={() => trackSignupCta("end-card")} size="lg" class="gap-2">
									Record your own
									<ArrowRight class="size-4" />
								</Button>
								<p class="text-[11px] text-white/50">Free · no credit card</p>
								<button
									type="button"
									onclick={replay}
									class="inline-flex items-center gap-1.5 text-xs font-medium text-white/60 transition-colors hover:text-white"
								>
									<RotateCcw class="size-3.5" />
									Replay
								</button>
							</div>
						</div>
					{/if}
				</div>
			</section>

			<!-- Floating action bar — sits just under the video. Id-based Lucide
			     reaction icons + on-demand Comments / Transcript triggers that open
			     the docked side panel. This is the only always-on chrome; the
			     conversation never competes with the video for space. -->
			<div class="relative z-20 mx-auto mt-4 flex w-fit max-w-full items-center gap-1 overflow-x-auto rounded-2xl border border-border-low bg-card p-1.5 shadow-craft-lg dark:shadow-(--shadow-craft-inset) backdrop-blur-xl">
				{#each REACTIONS as r (r.id)}
					{@const count = countFor(r.id)}
					{@const mine = myReactions.has(r.id)}
					<button
						type="button"
						onclick={() => react(r.id)}
						aria-pressed={mine}
						aria-label={r.label}
						title={r.label}
						class={cn(
							"group/react inline-flex shrink-0 items-center gap-1.5 rounded-xl px-2.5 py-1.5 text-sm transition-all",
							!mine && "hover:bg-foreground/5",
						)}
						style={mine ? `background-color: hsl(${r.hue} 85% 55% / 0.16)` : ""}
					>
						<ReactionIcon id={r.id} class="size-5 transition-transform group-hover/react:scale-110" />
						{#if count > 0}
							<span
								class={cn("font-mono text-[11px] tabular-nums", !mine && "text-muted-foreground")}
								style={mine ? `color: hsl(${r.hue} 60% 42%)` : ""}
							>{count}</span>
						{/if}
					</button>
				{/each}

				{#if commentsEnabled || hasTranscript}
					<span class="mx-1 h-6 w-px shrink-0 bg-border-low/60" aria-hidden="true"></span>
				{/if}

				{#if commentsEnabled}
					{@const commentsActive = panelOpen && activeTab === "comments"}
					<Button
						onclick={() => togglePanel("comments")}
						aria-pressed={commentsActive}
						variant={commentsActive ? "secondary" : "ghost"}
						size="sm"
					>
						<MessageSquare class="size-4" />
						<span class="max-sm:hidden">Comments</span>
						{#if comments.length > 0}
							<span class="rounded-md bg-foreground/10 px-1.5 py-0.5 font-mono text-[10px] tabular-nums">{comments.length}</span>
						{/if}
					</Button>
				{/if}
				{#if hasTranscript}
					{@const transcriptActive = panelOpen && activeTab === "transcript"}
					<Button
						onclick={() => togglePanel("transcript")}
						aria-pressed={transcriptActive}
						variant={transcriptActive ? "secondary" : "ghost"}
						size="sm"
					>
						<FileText class="size-4" />
						<span class="max-sm:hidden">Transcript</span>
					</Button>
				{/if}
			</div>

			<!-- Title + meta + description + CTA. Text stays a readable column even
			     though the video goes full-width. -->
			<div class="mx-auto mt-6 w-full max-w-3xl">
				<div class="flex items-start gap-2">
					<h1
						bind:this={titleAnchorEl}
						class="text-balance text-2xl font-semibold leading-tight tracking-tight sm:text-3xl"
					>
						{titleText}
					</h1>
					{#if canManage}
						<button
							type="button"
							onclick={openDetailsEditor}
							aria-label="Edit title and description"
							title="Edit details"
							class="mt-1 grid size-8 shrink-0 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
						>
							<PencilLine class="size-4" />
						</button>
					{/if}
				</div>
				<!-- Sender is the trust anchor — promoted to an avatar + name, with the
				     numbers (date · duration · views) as a lighter meta row beneath. -->
				<div class="mt-3 flex flex-wrap items-center gap-x-3 gap-y-2 text-xs text-muted-foreground">
					<span class="inline-flex items-center gap-2">
						<span class="grid size-6 shrink-0 place-items-center rounded-full bg-foreground/10 text-[9px] font-bold text-foreground ring-1 ring-border/40">
							{initials(recast?.sharedBy, null)}
						</span>
						<span class="text-foreground/90">Shared by <span class="font-semibold text-foreground">{recast?.sharedBy}</span></span>
					</span>
					<span aria-hidden="true" class="text-border-low">·</span>
					<span>{recast ? new Date(recast.sharedAt).toLocaleDateString(undefined, { month: "short", day: "numeric" }) : ""}</span>
					{#if recast?.durationSec}
						<span aria-hidden="true" class="text-border-low">·</span>
						<span class="font-mono tabular-nums">{formatTime(recast.durationSec)}</span>
					{/if}
					{#if viewsCount > 0}
						<span aria-hidden="true" class="text-border-low">·</span>
						<span class="inline-flex items-center gap-1">
							<Eye class="size-3" />
							{viewsCount.toLocaleString()} {viewsCount === 1 ? "view" : "views"}
						</span>
					{/if}
				</div>

				{#if descriptionText}
					<p class="group/desc mt-3 text-sm leading-relaxed text-muted-foreground">
						{descriptionText}
						{#if canManage}
							<button
								type="button"
								onclick={openDetailsEditor}
								class="ml-1.5 inline-flex items-center gap-1 align-middle text-xs font-medium text-muted-foreground/70 opacity-0 transition-opacity hover:text-foreground focus-visible:opacity-100 group-hover/desc:opacity-100"
							>
								<PencilLine class="size-3" /> Edit
							</button>
						{/if}
					</p>
				{:else if canManage}
					<button
						type="button"
						onclick={openDetailsEditor}
						class="mt-3 inline-flex items-center gap-2 rounded-xl border border-dashed border-border-low/70 px-3.5 py-2 text-xs font-medium text-muted-foreground transition-colors hover:border-primary/50 hover:text-foreground"
					>
						<PencilLine class="size-3.5" />
						Add a description
					</button>
				{/if}

				<!-- Persistent CTA — the founder's "next step", always visible (the
				     end-card only catches viewers who finish). -->
				{#if cta}
					<div class="mt-4">
						<Button href={cta.url} target="_blank" rel="noopener" class="gap-2">
							{cta.label}
							<ExternalLink class="size-3.5" />
						</Button>
					</div>
				{:else if canManage}
					<button
						type="button"
						onclick={openCtaEditor}
						class="mt-4 inline-flex items-center gap-2 rounded-xl border border-dashed border-border-low/70 px-3.5 py-2 text-xs font-medium text-muted-foreground transition-colors hover:border-primary/50 hover:text-foreground"
					>
						<Megaphone class="size-3.5" />
						Add a call-to-action
					</button>
				{:else if strangerLoop}
					<!-- Always-visible positioning for a cold visitor: says what Recast
					     is (most have never heard of it) and offers the path, without
					     waiting for the end-card they may never reach. -->
					<div class="mt-4 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
						<span class="text-muted-foreground">Recast is a free screen recorder.</span>
						<a
							href="/signup"
							onclick={() => trackSignupCta("positioning-chip")}
							class="inline-flex items-center gap-1 font-medium text-primary hover:underline"
						>
							Record your own
							<ArrowRight class="size-3.5" />
						</a>
					</div>
				{/if}
			</div>

				<!-- Engagement rail — in-layout and collapsible. On desktop it's a
				     sticky right-hand column (grid placement lives on <main>); on
				     mobile it stacks below the video. Toggled from the reaction-bar
				     Comments/Transcript buttons — the video reclaims full width when
				     it's collapsed. Name-only: viewers comment without an account. -->
			{#if hasSidebar}
				<aside
					bind:this={railEl}
					class="mt-4 flex h-[70vh] flex-col overflow-hidden rounded-2xl border border-border-low/50 bg-background/70 shadow-craft-lg backdrop-blur-xl lg:sticky lg:top-19 lg:mt-0 lg:h-[calc(100dvh-100px)]"
				>
					<div class="flex h-full flex-col overflow-hidden">
						<!-- Tab bar + close -->
						<div role="tablist" aria-label="Transcript and comments" class="flex shrink-0 items-center gap-1 border-b border-border-low/40 p-2">
							{#if hasTranscript}
								<Button
									role="tab"
									aria-selected={activeTab === "transcript"}
									onclick={() => (activeTab = "transcript")}
									variant={activeTab === "transcript" ? "secondary" : "ghost"}
									size="sm"
									class="flex-1"
								>
									<FileText class="size-3.5" />
									Transcript
								</Button>
							{/if}
							{#if commentsEnabled}
								<Button
									role="tab"
									aria-selected={activeTab === "comments"}
									onclick={() => (activeTab = "comments")}
									variant={activeTab === "comments" ? "secondary" : "ghost"}
									size="sm"
									class="flex-1"
								>
									<MessageSquare class="size-3.5" />
									Comments
									{#if comments.length > 0}
										<span class="rounded-md bg-foreground/10 px-1.5 py-0.5 font-mono text-[10px] tabular-nums">{comments.length}</span>
									{/if}
								</Button>
							{/if}
							<Button
								onclick={() => (panelOpen = false)}
								aria-label="Close panel"
								variant="ghost"
								size="icon-sm"
								class="ml-auto"
							>
								<X class="size-4" />
							</Button>
						</div>

						<!-- Transcript tab — cues read off the player's live caption
						     track; the current line lights and auto-scrolls, click to
						     seek, search to filter. -->
						{#if hasTranscript && activeTab === "transcript"}
							<div class="flex min-h-0 flex-1 flex-col">
								<div class="shrink-0 border-b border-border-low/40 p-2.5">
									<div class="relative">
										<Search class="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
										<input
											bind:value={transcriptQuery}
											type="text"
											placeholder="Search transcript…"
											class="w-full rounded-lg border border-border-low/70 bg-background/80 py-2 pl-8 pr-3 text-xs text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
										/>
									</div>
								</div>
								<div bind:this={transcriptListEl} class="min-h-0 flex-1 overflow-y-auto p-1.5">
									{#if transcriptCues.length === 0}
										<p class="px-2 py-8 text-center text-xs text-muted-foreground">Preparing transcript…</p>
									{:else if filteredCues.length === 0}
										<p class="px-2 py-8 text-center text-xs text-muted-foreground">No lines match “{transcriptQuery}”.</p>
									{:else}
										{#each filteredCues as cue, i (cue.id)}
											{@const isActive = !transcriptQuery.trim() && activeCue === i}
											<button
												type="button"
												data-cue={i}
												onclick={() => jumpTo(cue.start)}
												class={cn(
													"group/cue flex w-full gap-2.5 rounded-lg px-2 py-1.5 text-left transition-colors",
													isActive ? "bg-primary/10" : "hover:bg-foreground/5",
												)}
											>
												<span class={cn("shrink-0 pt-px font-mono text-[10px] tabular-nums", isActive ? "text-primary" : "text-muted-foreground group-hover/cue:text-foreground")}>
													{formatTime(cue.start)}
												</span>
												<span class={cn("text-[13px] leading-relaxed", isActive ? "text-foreground" : "text-foreground/70")}>
													{cue.text}
												</span>
											</button>
										{/each}
									{/if}
								</div>
							</div>
						{/if}

						<!-- Comments tab — thread scrolls, composer pinned to the
						     bottom so it's always reachable beside the video. -->
						{#if commentsEnabled && activeTab === "comments"}
							<div class="flex min-h-0 flex-1 flex-col">
								<div class="min-h-0 flex-1 overflow-y-auto p-2">
									{#if engagementState === "loading"}
									<!-- Skeleton rows so an opened panel reads as loading, not
									     empty, during the client-side engagement fetch. -->
										<ul class="animate-pulse space-y-1" aria-hidden="true">
											{#each [0, 1, 2] as i (i)}
												<li class="flex items-start gap-3 px-2 py-3">
													<span class="size-8 shrink-0 rounded-full bg-foreground/8"></span>
													<div class="min-w-0 flex-1 space-y-2">
														<div class="h-3 w-24 rounded bg-foreground/8"></div>
														<div class="h-3 w-full rounded bg-foreground/6"></div>
														<div class="h-3 w-3/5 rounded bg-foreground/6"></div>
													</div>
												</li>
											{/each}
										</ul>
									{:else if engagementState === "error"}
										<div class="flex flex-col items-center gap-3 px-4 py-10 text-center">
											<p class="text-sm text-muted-foreground">Couldn't load the conversation.</p>
											<Button size="sm" variant="outline" onclick={retryEngagement} class="gap-1.5">
												<RotateCcw class="size-3.5" />
												Try again
											</Button>
										</div>
									{:else if comments.length === 0}
										<p class="px-1 py-8 text-center text-sm text-muted-foreground">No comments yet. Be the first.</p>
									{:else}
										<ul>
											{#each comments as c (c.id)}
												{@const within = Math.abs(c.atSeconds - smoothedTime.current) < 5}
												<li
													id={`comment-${c.id}`}
													class={cn(
														"group/comment flex items-start gap-3 rounded-xl px-2 py-3 transition-colors",
														within ? "bg-primary/8" : "hover:bg-foreground/4",
													)}
												>
													<button
														type="button"
														onclick={() => jumpTo(c.atSeconds)}
														aria-label="Jump to {c.authorName}'s comment at {formatTime(c.atSeconds)}"
														class={cn(
															"grid size-8 shrink-0 place-items-center rounded-full text-[11px] font-bold ring-1 transition-transform hover:scale-105",
															c.verified ? "bg-foreground text-background ring-border/40" : "text-white ring-transparent",
														)}
														style={c.verified ? "" : `background: hsl(${commentHue(c.authorName)} 60% 45%);`}
													>
														{c.authorName[0]?.toUpperCase()}
													</button>
													<div class="min-w-0 flex-1">
														<div class="flex items-center gap-2">
															<span class="text-sm font-semibold">{c.authorName}</span>
															{#if c.verified}
																<span class="inline-flex shrink-0 items-center text-primary" title="Signed-in account">
																	<BadgeCheck class="size-3.5" />
																	<span class="sr-only">Verified account</span>
																</span>
															{/if}
															<button
																type="button"
																onclick={() => jumpTo(c.atSeconds)}
																class={cn("font-mono text-[10px] tabular-nums transition-colors hover:text-primary", within ? "text-primary" : "text-muted-foreground")}
															>
																{formatTime(c.atSeconds)}
															</button>
															{#if c.mine || canManage}
																<button
																	type="button"
																	onclick={() => removeComment(c.id)}
																	aria-label="Delete comment"
																	class="ml-auto grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground opacity-0 transition-all hover:bg-destructive/10 hover:text-destructive focus-visible:opacity-100 group-hover/comment:opacity-100"
																>
																	<Trash2 class="size-3" />
																</button>
															{/if}
														</div>
														<p class="mt-0.5 text-[13px] leading-relaxed text-foreground/85">
															{#each parseCommentText(c.body) as seg, i (i)}
																{#if seg.kind === "text"}<!--
																-->{seg.text}<!--
															-->{:else if seg.kind === "timestamp"}<!--
																--><button
																	type="button"
																	onclick={() => jumpTo(seg.seconds)}
																	class="mx-px inline-flex -translate-y-px items-center px-1 align-middle text-[11px] font-medium text-primary transition-colors hover:underline"
																>{formatTime(seg.seconds)}</button><!--
															-->{:else if seg.kind === "mention"}<!--
																--><span class="mx-px rounded-md bg-foreground/10 px-1 font-medium text-foreground">@{seg.name}</span><!--
															-->{/if}
															{/each}
														</p>
													</div>
												</li>
											{/each}
										</ul>
									{/if}
								</div>
								<!-- Composer (pinned). Comment input is the focal point (full
								     width); identity is secondary — a remembered "commenting as"
								     line that only expands to a field when unset or edited. -->
								<div class="shrink-0 border-t border-border-low/40 p-2.5">
									<div class="flex flex-col gap-2">
										{#if identityLocked}
											<!-- Signed in: the account IS the identity. No name field,
											     no Change — only guests choose a name. -->
											<div class="flex items-center gap-1.5 px-0.5 text-[11px] text-muted-foreground">
												<span class="grid size-4 shrink-0 place-items-center overflow-hidden rounded-full bg-foreground text-[7px] font-bold text-background ring-1 ring-border/40">
													{#if viewer?.image}
														<img src={viewer.image} alt="" referrerpolicy="no-referrer" class="size-full object-cover" />
													{:else}
														{viewerInitials}
													{/if}
												</span>
												<span>Commenting as <span class="font-medium text-foreground">{viewerName}</span></span>
												<span
													class="ml-0.5 inline-flex items-center gap-0.5 rounded-full bg-foreground/8 px-1.5 py-px text-[9px] font-medium text-muted-foreground ring-1 ring-border/40"
													title="Signed in — comments use your account name"
												>
													<UserCheck class="size-2.5" />
													Account
												</span>
											</div>
										{:else if viewerName.trim() && !editingName}
											<div class="flex items-center gap-1.5 px-0.5 text-[11px] text-muted-foreground">
												<span
													class="grid size-4 shrink-0 place-items-center rounded-full text-[8px] font-bold text-white"
													style="background: hsl({commentHue(viewerName)} 60% 45%);"
												>{viewerName.trim()[0]?.toUpperCase()}</span>
												<span>Commenting as <span class="font-medium text-foreground">{viewerName.trim()}</span></span>
												<button
													type="button"
													onclick={editName}
													class="ml-0.5 font-medium text-muted-foreground/80 underline-offset-2 transition-colors hover:text-foreground hover:underline"
												>
													Change
												</button>
											</div>
										{:else}
											<input
												bind:this={nameInputEl}
												bind:value={viewerName}
												type="text"
												placeholder="Your name"
												maxlength="60"
												onblur={commitName}
												onkeydown={(e) => {
													if (e.key === "Enter") {
														e.preventDefault();
														commitName();
														inputEl?.focus();
													}
												}}
												class="w-full max-w-[220px] rounded-lg border border-border-low/60 bg-background/60 px-3 py-1.5 text-xs text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
											/>
										{/if}

										<input
											bind:this={inputEl}
											bind:value={draftText}
											type="text"
											placeholder="Add a comment at {formatTime(currentTime)}…"
											onkeydown={(e) => {
												if (e.key === "Enter" && !e.shiftKey) {
													e.preventDefault();
													submitComment();
												}
											}}
											class="w-full rounded-lg border border-border-low/70 bg-background/80 px-3 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
										/>

										<div class="flex items-center gap-2">
											<p class="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-0.5 text-[10px] text-muted-foreground">
												<span class="inline-flex items-center gap-1"><Clock class="size-2.5" /><span class="font-mono">[m:ss]</span> jumps</span>
												<span aria-hidden="true">·</span>
												<span class="inline-flex items-center gap-1"><AtSign class="size-2.5" /><span class="font-mono">@name</span> mentions</span>
											</p>
											<Tooltip.Provider delayDuration={250}>
												<Tooltip.Root>
													<Tooltip.Trigger
														onclick={insertCurrentTimestamp}
														aria-label="Insert current timestamp"
														class={cn(buttonVariants({ variant: "outline", size: "icon-sm" }), "text-muted-foreground")}
													>
														<Clock class="size-3.5" />
													</Tooltip.Trigger>
													<Tooltip.Content sideOffset={8}>
														<span class="text-[11px]">Insert <span class="font-mono">[{formatTime(currentTime)}]</span></span>
													</Tooltip.Content>
												</Tooltip.Root>
											</Tooltip.Provider>
											<Button size="sm" class="shrink-0 gap-1.5" disabled={!draftText.trim() || !viewerName.trim()} onclick={submitComment}>
												Post
												<ArrowRight class="size-3.5" />
											</Button>
										</div>
									</div>
								</div>
							</div>
						{/if}
					</div>
				</aside>
			{/if}


			<!-- Mid-watch nudge: a subtle floating pill, fixed at bottom-center,
			     shown once at ~50% watched for a cold visitor. Dismissible, out of
			     flow so it never shoves the video, and suppressed in fullscreen so it
			     never intrudes on an immersive watch. -->
			{#if showMidWatchNudge}
				<div
					class="fixed inset-x-0 bottom-4 z-40 mx-auto flex w-fit max-w-[calc(100%-2rem)] items-center gap-3 rounded-2xl border border-primary/30 bg-card/95 px-3.5 py-2.5 shadow-craft-xl backdrop-blur-xl"
					in:fly={{ y: 16, duration: 260, easing: cubicOut }}
					out:fade={{ duration: 160 }}
					role="complementary"
					aria-label="Try Recast"
				>
					<span
						class="grid size-8 shrink-0 place-items-center rounded-xl bg-foreground/5 text-foreground ring-1 ring-border/40"
						aria-hidden="true"
					>
						<Logo size="18" color="transparent" fill="currentColor" />
					</span>
					<p class="text-[13px] text-foreground">
						Like this? <span class="text-muted-foreground">Make your own, free.</span>
					</p>
					<Button
						href="/signup"
						onclick={() => trackSignupCta("mid-watch")}
						size="sm"
						class="shrink-0 gap-1.5"
					>
						Try Recast free
						<ArrowRight class="size-3.5" />
					</Button>
					<button
						type="button"
						onclick={dismissNudge}
						aria-label="Dismiss"
						class="grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
					>
						<X class="size-3.5" />
					</button>
				</div>
			{/if}
		</main>
	</div>

	<!-- Owner CTA editor — opened from the Share menu, kept off the viewer
	     surface entirely. -->
	{#if canManage}
		<Dialog.Root bind:open={ctaDialogOpen}>
			<Dialog.Content>
				<Dialog.Header>
					<Dialog.Title class="flex items-center gap-2">
						<span class="glass-chip grid size-7 place-items-center rounded-lg text-primary">
							<Megaphone class="size-3.5" />
						</span>
						Call-to-action
					</Dialog.Title>
					<Dialog.Description>
						The next step you want viewers to take. Shown as a button below the video and when it ends.
					</Dialog.Description>
				</Dialog.Header>
				<form class="space-y-3" onsubmit={saveCta}>
					<Label class="block">
						<span class="mb-1 block text-xs font-semibold text-foreground/85">Button text</span>
						<Input bind:value={ctaLabelDraft} placeholder="Book a 15-min call" maxlength={60} class="h-10" />
					</Label>
					<Label class="block">
						<span class="mb-1 block text-xs font-semibold text-foreground/85">Link</span>
						<Input bind:value={ctaUrlDraft} placeholder="https://cal.com/you/intro" type="url" class="h-10" />
					</Label>
					<Dialog.Footer class="gap-2">
						{#if cta}
							<Button
								type="button"
								variant="ghost"
								class="mr-auto text-destructive hover:bg-destructive/10 hover:text-destructive"
								onclick={() => {
									ctaLabelDraft = "";
									ctaUrlDraft = "";
								}}
							>
								Clear
							</Button>
						{/if}
						<Button type="button" variant="ghost" onclick={() => (ctaDialogOpen = false)}>Cancel</Button>
						<Button type="submit" disabled={savingCta} class="gap-2">
							{savingCta ? "Saving…" : "Save"}
							{#if !savingCta}<Check class="size-4" />{/if}
						</Button>
					</Dialog.Footer>
				</form>
			</Dialog.Content>
		</Dialog.Root>

		<!-- Owner details editor — the recast's own title + blurb, shown on the
		     page and reused as the OG/social-card text. -->
		<Dialog.Root bind:open={detailsOpen}>
			<Dialog.Content>
				<Dialog.Header>
					<Dialog.Title class="flex items-center gap-2">
						<span class="glass-chip grid size-7 place-items-center rounded-lg text-primary">
							<PencilLine class="size-3.5" />
						</span>
						Edit details
					</Dialog.Title>
					<Dialog.Description>
						The title and blurb shown on the video and in the link preview when this recast is shared.
					</Dialog.Description>
				</Dialog.Header>
				<form class="space-y-3" onsubmit={saveDetails}>
					<Label class="block">
						<span class="mb-1 block text-xs font-semibold text-foreground/85">Title</span>
						<Input bind:value={titleDraft} placeholder="Untitled recast" maxlength={200} class="h-10" required />
					</Label>
					<Label class="block">
						<span class="mb-1 block text-xs font-semibold text-foreground/85">Description</span>
						<textarea
							bind:value={descDraft}
							rows="4"
							maxlength={500}
							placeholder="What's this recording about?"
							class="w-full resize-none rounded-lg border border-border-low/70 bg-background/80 px-3 py-2 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
						></textarea>
					</Label>
					<Dialog.Footer class="gap-2">
						{#if descriptionText}
							<Button
								type="button"
								variant="ghost"
								class="mr-auto text-destructive hover:bg-destructive/10 hover:text-destructive"
								onclick={() => (descDraft = "")}
							>
								Clear description
							</Button>
						{/if}
						<Button type="button" variant="ghost" onclick={() => (detailsOpen = false)}>Cancel</Button>
						<Button type="submit" disabled={savingDetails || !titleDraft.trim()} class="gap-2">
							{savingDetails ? "Saving…" : "Save"}
							{#if !savingDetails}<Check class="size-4" />{/if}
						</Button>
					</Dialog.Footer>
				</form>
			</Dialog.Content>
		</Dialog.Root>
	{/if}
{/if}


<style>
	/* Engagement rail open/close. The grid's second track is `auto`, so it
	   follows the rail's own width — animating the rail's width + margin eases
	   the video (the 1fr track) to its new size as the rail reveals. Width and
	   margin transitions are universally supported, unlike grid-track interp. */
	@media (min-width: 1024px) {
		:global(.share-main[data-has-rail="true"]) {
			display: grid;
			grid-template-columns: minmax(0, 1fr) auto;
			column-gap: 0;
			align-items: start;
		}
		:global(.share-main[data-has-rail="true"] > *:not(aside)) {
			grid-column: 1;
		}
		:global(.share-main[data-has-rail="true"] > aside) {
			grid-column: 2;
			grid-row: 1 / span 99;
			width: 0;
			margin-left: 0;
			overflow: hidden;
			opacity: 0;
			pointer-events: none;
			transition:
				width 320ms cubic-bezier(0.4, 0, 0.2, 1),
				margin-left 320ms cubic-bezier(0.4, 0, 0.2, 1),
				opacity 240ms ease;
		}
		:global(.share-main[data-rail="open"] > aside) {
			width: 360px;
			margin-left: 1.5rem;
			opacity: 1;
			pointer-events: auto;
		}
		/* Fixed inner width so the rail clips as it grows/shrinks rather than
		   reflowing its contents mid-animation. */
		:global(.share-main[data-has-rail="true"] > aside > *) {
			width: 360px;
		}
	}
	@media (max-width: 1023px) {
		:global(.share-main[data-rail="closed"] > aside) {
			display: none;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		:global(.share-main[data-has-rail="true"] > aside) {
			transition: none;
		}
	}
</style>
