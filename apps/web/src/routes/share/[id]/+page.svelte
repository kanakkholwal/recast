<script lang="ts">
	import { browser } from "$app/environment";
	import { page } from "$app/state";
	import Logo from "$lib/logo.svelte";
	import {
	  ArrowRight,
	  AtSign,
	  Check,
	  Clock,
	  Code,
	  Copy,
	  Download,
	  Film,
	  Globe,
	  LayoutDashboard,
	  Link2,
	  Lock,
	  LogOut,
	  Mail,
	  MessageSquare,
	  Moon,
	  Pause,
	  Play,
	  RectangleHorizontal,
	  ScrollText,
	  Settings,
	  Share2,
	  ShieldOff,
	  Square,
	  Sun,
	  User,
	  Users,
	} from "@lucide/svelte";
	import { goto } from "$app/navigation";
	import { authClient } from "$lib/auth/client";
	// Alias because the local `mode` ($state for view mode) collides with
	// mode-watcher's exported reactor of the same name.
	import { mode as themeMode, toggleMode } from "@recast/ui/theme";
	import { RecastPlayer, type RecastPlayerApi } from "@recast/player";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { toast } from "@recast/ui/sonner";
	import * as Tooltip from "@recast/ui/tooltip";
	import { cn } from "@recast/ui/utils";
	import { onMount, untrack } from "svelte";
	import { cubicOut, quintOut } from "svelte/easing";
	import { Tween } from "svelte/motion";
	import { fade, fly, scale, slide } from "svelte/transition";

	let { data } = $props();

	// `access` is the server-resolved permission envelope. When `ok` is
	// true it carries the recast + share + canManage; when false it
	// carries the reason + (for same-team viewers) the owner contact so
	// the denial card can render a "request access" affordance. We expose
	// a typed alias for the ok branch so the rest of the script reads
	// `recast.title` instead of `data.access.recast.title`.
	const access = $derived(data.access);
	const okAccess = $derived(access.ok ? access : null);
	const deniedAccess = $derived(access.ok ? null : access);
	const recast = $derived(okAccess?.recast);
	const shareMeta = $derived(okAccess?.share);
	const canManage = $derived(okAccess?.canManage ?? false);
	let visibility = $state<"public" | "team" | "private">(
		untrack(() => (data.access.ok ? data.access.share.visibility : "public")),
	);
	let updatingVisibility = $state(false);
	$effect(() => {
		if (access.ok) visibility = access.share.visibility;
	});

	async function updateVisibility(next: "public" | "team" | "private") {
		if (!shareMeta || !canManage || updatingVisibility) return;
		if (next === visibility) return;
		const previous = visibility;
		visibility = next;
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
						const message = await res.text().catch(() => "Couldn't update access");
						throw new Error(message || "Couldn't update access");
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
			// Roll back the optimistic update; the toast already surfaced the error.
			visibility = previous;
		} finally {
			updatingVisibility = false;
		}
	}

	/**
	 * Four viewing modes — picked to cover the different reasons someone
	 * lands on a share link:
	 *
	 *   • focus   — video as the message; minimal chrome around it
	 *   • theater — wide video with description right below, YouTube-ish
	 *   • review  — transcript + comments tabbed in a side rail (the work
	 *               surface for feedback rounds)
	 *   • cinema  — full-bleed, controls auto-hide, distraction-free
	 *
	 * Mode persists per-viewer in localStorage and can be pinned via
	 * `?view=` for share-link presets ("send this to a stakeholder in
	 * cinema mode").
	 */
	type Mode = "focus" | "theater" | "review" | "cinema";
	type ReviewTab = "transcript" | "comments";

	const MODES: { id: Mode; label: string; icon: typeof Square; hint: string }[] = [
		{ id: "focus", label: "Focus", icon: Square, hint: "Centered, no extras" },
		{ id: "theater", label: "Theater", icon: RectangleHorizontal, hint: "Wide video, details below" },
		{ id: "review", label: "Review", icon: MessageSquare, hint: "Transcript + comments rail" },
		{ id: "cinema", label: "Cinema", icon: Film, hint: "Full-bleed, distraction-free" },
	];

	/**
	 * Coerce a raw URL/localStorage value into a current Mode. We honor
	 * legacy `comments` / `transcript` values by folding them into
	 * `review` (and remembering which tab they meant) so old share links
	 * keep working after the merge.
	 */
	function coerceMode(raw: string | null | undefined): { mode: Mode; tab: ReviewTab | null } {
		if (!raw) return { mode: "theater", tab: null };
		if (raw === "comments") return { mode: "review", tab: "comments" };
		if (raw === "transcript") return { mode: "review", tab: "transcript" };
		if (MODES.some((m) => m.id === raw)) return { mode: raw as Mode, tab: null };
		return { mode: "theater", tab: null };
	}

	function readInitialMode(): Mode {
		return coerceMode(page.url.searchParams.get("view")).mode;
	}

	function readInitialReviewTab(): ReviewTab {
		const urlTab = page.url.searchParams.get("tab");
		if (urlTab === "comments" || urlTab === "transcript") return urlTab;
		// Honor a legacy `view=comments|transcript` even though we
		// rewrite `view` to `review`.
		const legacy = coerceMode(page.url.searchParams.get("view")).tab;
		if (legacy) return legacy;
		return "transcript";
	}

	/**
	 * Parse YouTube-style time params. Accepted forms:
	 *   ?t=120       → 120s
	 *   ?t=120s      → 120s
	 *   ?t=2m30s     → 150s
	 *   ?t=1h2m30s   → 3750s
	 *   ?t=1h        → 3600s
	 * Anything we can't parse returns 0 (no seek).
	 */
	function parseTimeParam(raw: string | null): number {
		if (!raw) return 0;
		const t = raw.trim().toLowerCase();
		if (/^\d+$/.test(t)) return Number(t);
		let total = 0;
		const h = t.match(/(\d+)h/);
		const m = t.match(/(\d+)m/);
		const s = t.match(/(\d+)s/);
		if (h) total += Number(h[1]) * 3600;
		if (m) total += Number(m[1]) * 60;
		if (s) total += Number(s[1]);
		return total;
	}

	let mode = $state<Mode>(untrack(readInitialMode));
	let reviewTab = $state<ReviewTab>(untrack(readInitialReviewTab));
	const initialSeekSeconds = untrack(() => parseTimeParam(page.url.searchParams.get("t")));

	onMount(() => {
		// Restore persisted mode unless the URL pinned one. Legacy
		// `comments` / `transcript` values fold into `review` here too.
		if (!page.url.searchParams.get("view") && browser) {
			const stored = localStorage.getItem("recast.share.mode");
			const coerced = coerceMode(stored);
			if (stored && coerced.mode !== "theater") mode = coerced.mode;
			if (coerced.tab) reviewTab = coerced.tab;
		}
	});

	$effect(() => {
		if (!browser) return;
		// URL is the source of truth — every mode change updates both the
		// query string (via replaceState so the back button skips noise)
		// and localStorage (so a viewer's last-used mode survives a hard
		// refresh on a URL without `?view=`). `theater` is the default and
		// stays out of the URL to keep clean links clean. `tab` only
		// rides along when we're in review mode.
		localStorage.setItem("recast.share.mode", mode);
		const params = new URLSearchParams(window.location.search);
		if (mode === "theater") params.delete("view");
		else params.set("view", mode);
		if (mode === "review") params.set("tab", reviewTab);
		else params.delete("tab");
		const search = params.toString();
		const newUrl = `${window.location.pathname}${search ? `?${search}` : ""}`;
		window.history.replaceState({}, "", newUrl);
	});

	let api = $state<RecastPlayerApi | null>(null);

	// Re-seek every time the player API publishes — covers both the
	// initial `?t=` URL seed AND subsequent mounts that happen when
	// cinema mode flips (the player remounts because cinema lives in a
	// different DOM branch). Reading `currentTime` via `untrack` so this
	// effect only re-runs when `api` itself changes, not on every
	// playback progress tick.
	$effect(() => {
		if (!api) return;
		const target = untrack(() => Math.max(initialSeekSeconds, currentTime));
		if (target > 0) api.seek(target);
	});

	// Real-time playhead — exposed so the transcript / comments rail can
	// highlight the active cue. A Tween smooths jitter between
	// `time-update` events (which only fire ~4× per second).
	let currentTime = $state(initialSeekSeconds);
	const smoothedTime = new Tween(0, { duration: 120, easing: cubicOut });
	let watchedPct = $state(0);
	let isPlaying = $state(false);

	$effect(() => {
		smoothedTime.set(currentTime);
	});

	// ── Demo content keyed to the 596s Big Buck Bunny stream ───────────

	type Comment = {
		id: string;
		author: string;
		avatarHue: number;
		atSeconds: number;
		text: string;
	};
	let comments = $state<Comment[]>([
		{ id: "c1", author: "Mia", avatarHue: 200, atSeconds: 14, text: "Love the opening shot at [0:14] — the lighting translates well to compressed playback." },
		{ id: "c2", author: "Dev", avatarHue: 340, atSeconds: 52, text: "@Mia can we lift the audio bed by ~3dB at [0:52]? Voice is competing." },
		{ id: "c3", author: "Sara", avatarHue: 90, atSeconds: 128, text: "@Kai this cut at 2:08 is sharp. Worth showing the founder the full sequence." },
		{ id: "c4", author: "Kai", avatarHue: 270, atSeconds: 246, text: "Pacing dips around [4:06] — maybe trim 4-5 seconds. @Sara good catch on the earlier one." },
		{ id: "c5", author: "Mia", avatarHue: 200, atSeconds: 380, text: "Color grade at [6:20] matches the deck — nice." },
		{ id: "c6", author: "Dev", avatarHue: 340, atSeconds: 540, text: "End card at 9:00 lingers a touch long. Otherwise approved." },
	]);

	/**
	 * Rich-text segments for a comment body. We support two markdown-like
	 * tokens because viewers will type whichever feels natural:
	 *
	 *   - `[m:ss]` / `[h:mm:ss]` — explicit, bracketed timestamp link
	 *   - `m:ss` / `h:mm:ss`     — loose timestamp (must have word boundaries)
	 *   - `@name`                — user mention (visual only for now;
	 *                              notification wiring comes later)
	 *
	 * The renderer below turns each timestamp into a clickable chip that
	 * drives `jumpTo()`, and each mention into a styled inline pill.
	 */
	type CommentSegment =
		| { kind: "text"; text: string }
		| { kind: "timestamp"; seconds: number; raw: string }
		| { kind: "mention"; name: string };

	function parseTimeToken(s: string): number {
		const parts = s.split(":").map((p) => Number(p));
		if (parts.some(Number.isNaN)) return 0;
		if (parts.length === 2) return parts[0]! * 60 + parts[1]!;
		if (parts.length === 3) return parts[0]! * 3600 + parts[1]! * 60 + parts[2]!;
		return 0;
	}

	function parseCommentText(text: string): CommentSegment[] {
		// Bracketed timestamp wins over loose match — we tag the bracketed
		// regex first by listing it as the leftmost alternative.
		const re =
			/\[(\d{1,2}(?::\d{2}){1,2})\]|\b(\d{1,2}:\d{2}(?::\d{2})?)\b|@([A-Za-z][\w]{0,31})/g;
		const out: CommentSegment[] = [];
		let lastIdx = 0;
		let m: RegExpExecArray | null;
		while ((m = re.exec(text)) !== null) {
			if (m.index > lastIdx) {
				out.push({ kind: "text", text: text.slice(lastIdx, m.index) });
			}
			if (m[1] !== undefined) {
				out.push({ kind: "timestamp", seconds: parseTimeToken(m[1]), raw: m[1] });
			} else if (m[2] !== undefined) {
				out.push({ kind: "timestamp", seconds: parseTimeToken(m[2]), raw: m[2] });
			} else if (m[3] !== undefined) {
				out.push({ kind: "mention", name: m[3] });
			}
			lastIdx = m.index + m[0].length;
		}
		if (lastIdx < text.length) {
			out.push({ kind: "text", text: text.slice(lastIdx) });
		}
		return out;
	}

	// Draft state for the reply input. Wired so the "Add timestamp" button
	// can insert `[m:ss]` at the cursor and `Post` can publish locally.
	let draftText = $state("");
	let inputEl = $state<HTMLInputElement | null>(null);

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
		// Restore focus + cursor after Svelte applies the new value.
		queueMicrotask(() => {
			el.focus();
			const pos = start + stamp.length;
			el.setSelectionRange(pos, pos);
		});
	}

	function postComment() {
		const text = draftText.trim();
		if (!text) return;
		comments = [
			...comments,
			{
				id: `local-${Date.now()}`,
				author: "You",
				avatarHue: 50, // brand-lime-ish — distinguishes the viewer's own posts
				atSeconds: Math.floor(currentTime),
				text,
			},
		];
		draftText = "";
		toast.success(`Posted at ${formatTime(currentTime)}.`);
	}

	type Cue = { start: number; end: number; text: string };
	const transcript: Cue[] = [
		{ start: 0, end: 8, text: "Welcome to the demo — this is what a finished walkthrough looks like in Recast." },
		{ start: 8, end: 22, text: "The pipeline starts in the desktop app: capture, trim, color, export." },
		{ start: 22, end: 38, text: "Everything we cut here is destructive-free; the source stays intact on disk." },
		{ start: 38, end: 60, text: "Once you're happy with the edit, you publish to Recast Cloud and get a share link." },
		{ start: 60, end: 80, text: "Recipients land on a page like this. They can leave timestamped comments." },
		{ start: 80, end: 110, text: "If you want, they can see the transcript alongside the video and click to seek." },
		{ start: 110, end: 145, text: "For external review, switch to cinema mode — full-screen, no distractions." },
		{ start: 145, end: 180, text: "We'll talk about the AI editor next: cut markers, auto-trim, B-roll suggestions." },
		{ start: 180, end: 240, text: "The same player ships in the desktop app and in the web share page." },
		{ start: 240, end: 300, text: "Adaptive bitrate over HLS keeps it watchable on patchy connections." },
		{ start: 300, end: 360, text: "Browser support for PiP and AirPlay is automatic — media-chrome handles the wiring." },
		{ start: 360, end: 420, text: "Engagement events stream back so you know who watched what, and how far." },
		{ start: 420, end: 480, text: "Coming soon: hover-scrub thumbnails generated at upload time." },
		{ start: 480, end: 540, text: "And richer comments — replies, reactions, and resolve states." },
		{ start: 540, end: 596, text: "That's the tour. Thanks for watching — let us know what you think." },
	];

	const activeCue = $derived(
		transcript.find((c) => smoothedTime.current >= c.start && smoothedTime.current < c.end) ?? null,
	);

	const activeCommentsCount = $derived(
		comments.filter((c) => Math.abs(c.atSeconds - smoothedTime.current) < 5).length,
	);

	function jumpTo(seconds: number) {
		api?.seek(seconds);
		if (!isPlaying) api?.play();
		// Reflect the deliberate navigation in the URL so a refresh (or
		// share at this moment) re-opens at the same spot. We use
		// `replaceState` so back-button history isn't polluted by every
		// transcript-cue / comment click.
		if (!browser) return;
		const url = new URL(window.location.href);
		const t = compactTime(seconds);
		if (t) url.searchParams.set("t", t);
		else url.searchParams.delete("t");
		window.history.replaceState({}, "", url.toString());
	}

	function togglePlay() {
		if (isPlaying) api?.pause();
		else api?.play();
	}

	function formatTime(sec: number): string {
		const m = Math.floor(sec / 60);
		const s = Math.floor(sec % 60);
		return `${m}:${String(s).padStart(2, "0")}`;
	}

	/**
	 * Build a YouTube-compatible compact time token: `1h2m30s`, dropping
	 * any zero-valued segments. `0s` is a no-op (returns "").
	 */
	function compactTime(sec: number): string {
		const s = Math.max(0, Math.floor(sec));
		if (s === 0) return "";
		const h = Math.floor(s / 3600);
		const m = Math.floor((s % 3600) / 60);
		const r = s % 60;
		let out = "";
		if (h) out += `${h}h`;
		if (m) out += `${m}m`;
		if (r || !out) out += `${r}s`;
		return out;
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
		const url = new URL(window.location.href);
		url.searchParams.delete("t");
		await writeClipboard(url.toString(), "Share link copied.");
	}

	async function copyLinkAtCurrentTime() {
		if (!browser) return;
		const url = new URL(window.location.href);
		const t = compactTime(currentTime);
		if (t) url.searchParams.set("t", t);
		else url.searchParams.delete("t");
		await writeClipboard(
			url.toString(),
			t ? `Link copied at ${formatTime(currentTime)}.` : "Link copied.",
		);
	}

	async function copyEmbedCode() {
		if (!browser) return;
		const url = new URL(window.location.href);
		// Force cinema mode in embeds so the iframe is the player itself,
		// not a tiny shrunk-down full-page chrome. Time fragment travels
		// along so an embed at a specific moment "just works".
		url.searchParams.set("view", "cinema");
		const iframe = `<iframe src="${url.toString()}" width="640" height="360" frameborder="0" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen></iframe>`;
		await writeClipboard(iframe, "Embed code copied.");
	}

	// ── Viewer identity ─────────────────────────────────────────────
	// Share page is public, but a signed-in viewer should see their own
	// avatar + a one-click path back to their dashboard. We use the
	// client-side session reactor so we don't block SSR on an auth call
	// — the avatar just pops in when the cookie resolves.
	type SessionShape = {
		data: {
			user?: {
				name?: string | null;
				email?: string | null;
				image?: string | null;
			} | null;
		} | null;
	};
	const session = authClient.useSession();
	const viewer = $derived(($session as unknown as SessionShape).data?.user ?? null);

	function initials(name: string | null | undefined, email: string | null | undefined): string {
		const src = (name ?? email ?? "?").trim();
		if (!src) return "?";
		const parts = src.split(/\s+/).filter(Boolean);
		if (parts.length >= 2) return (parts[0]![0]! + parts[1]![0]!).toUpperCase();
		return src.slice(0, 2).toUpperCase();
	}

	/**
	 * Stable color hash for the avatar — same input always lands on the
	 * same hue, so a viewer's avatar tile reads consistent across pages.
	 */
	function avatarHue(seed: string): number {
		let h = 0;
		for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0;
		return h % 360;
	}

	async function signOut() {
		await authClient.signOut();
		await goto("/");
	}

	// Layout math driven by the active mode. We do this in JS so the
	// values can be tween'd / animated; CSS Grid can't yet smoothly
	// interpolate template changes across browsers.
	const isFullBleed = $derived(mode === "cinema");
	const showRail = $derived(mode === "review");
	const wrapMaxWidth = $derived(
		mode === "focus" ? "max-w-3xl" :
		mode === "theater" ? "max-w-5xl" :
		mode === "review" ? "max-w-7xl" :
		"max-w-none",
	);

	// Scroll active transcript cue into view inside the rail.
	let transcriptListEl = $state<HTMLElement | null>(null);
	$effect(() => {
		if (!activeCue || !transcriptListEl) return;
		const el = transcriptListEl.querySelector<HTMLElement>(`[data-cue-start="${activeCue.start}"]`);
		if (el) el.scrollIntoView({ behavior: "smooth", block: "center" });
	});
</script>

<svelte:head>
	<title>{recast?.title ?? "Private recast"} - Recast</title>
	<meta name="description" content={recast?.description ?? ""} />
	<meta name="robots" content="noindex" />
</svelte:head>

{#if deniedAccess}
	<!-- Denial fallback. Same-team viewers get a "request access" CTA
	     that opens their mail client to the owner; strangers get a plain
	     "no access" card with no contact info (we don't leak owner emails
	     to arbitrary visitors). Signed-out viewers see a sign-in nudge —
	     a private/team link they own would unlock after sign-in. -->
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
						{#if deniedAccess.visibility === "team"}
							This recast is shared with a specific team. Ask the owner to add you, or sign in with an account that's a member.
						{:else if deniedAccess.visibility === "private"}
							This recast is private. Only the owner can view it.
						{:else}
							This share link isn't available to your account.
						{/if}
					</p>
				</div>
			</div>

			<div class="mt-5 flex flex-col gap-2">
				{#if deniedAccess.sameTeam && deniedAccess.ownerEmail}
					<!-- Same-team but still denied (private share). Surface the
					     owner email so the viewer can ask for access without
					     guessing who to ping. -->
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
		</div>
	</div>
{:else}
<!-- Cinema mode renders a completely different shell: full-bleed black
     canvas, controls auto-hide, no chrome around the video. The morph
     reads as a "zoom into the player" — backdrop fades to black while
     the inner stage scales up from 0.94 with a long quint-out tail
     (560ms) so the eye tracks the player into the canvas instead of
     the layout snapping. Both branches animate in AND out so they
     crossfade. `currentTime` is preserved across the remount via the
     api-watching effect above. -->
{#if isFullBleed}
	<div
		class="fixed inset-0 z-50 grid place-items-center bg-black"
		in:fade={{ duration: 420, easing: quintOut }}
		out:fade={{ duration: 280, easing: quintOut }}
	>
		<div
			class="relative w-full h-full grid place-items-center"
			in:scale={{ start: 0.94, duration: 560, easing: quintOut, opacity: 0.4 }}
			out:scale={{ start: 0.96, duration: 320, easing: quintOut, opacity: 0.6 }}
		>
			<div class="w-full max-w-screen max-h-screen aspect-video">
				<RecastPlayer
					bind:api
					src={recast!.src}
					poster={recast!.poster}
					title={recast!.title}
					onengagement={(e) => {
						if (e.type === "view-start") isPlaying = true;
						if (e.type === "progress") {
							currentTime = e.currentTime;
							watchedPct = e.percent;
						}
						if (e.type === "ended") {
							watchedPct = 100;
							isPlaying = false;
						}
					}}
				/>
			</div>
			<!-- Floating mode switcher. Cinema canvas is solid black in
			     both themes, so the pill is tuned against black: glassy
			     backdrop-blur container with a thin white hairline and
			     soft shadow, white/60 idle buttons, primary-tinted active
			     state. Keeps the same shape as the top-bar switcher so
			     viewers don't relearn it. -->
			<Tooltip.Provider delayDuration={250}>
				<div
					role="tablist"
					aria-label="View mode"
					class="absolute right-4 top-4 flex items-center gap-0.5 rounded-xl border border-white/10 bg-black/55 p-1 shadow-craft-md backdrop-blur-xl supports-[backdrop-filter]:bg-black/40"
					in:fly={{ y: -8, duration: 260, easing: cubicOut }}
				>
					{#each MODES as m (m.id)}
						<Tooltip.Root>
							<Tooltip.Trigger
								role="tab"
								aria-selected={mode === m.id}
								aria-label="{m.label} — {m.hint}"
								onclick={() => (mode = m.id)}
								class={cn(
									"grid size-8 place-items-center rounded-lg text-white/60 transition-all duration-200",
									"hover:bg-white/10 hover:text-white",
									mode === m.id && "bg-primary/20 text-primary ring-1 ring-primary/40",
								)}
							>
								<m.icon class="size-3.5" />
							</Tooltip.Trigger>
							<Tooltip.Content sideOffset={8} class="max-w-56 bg-popover">
								<div class="text-[11px] leading-snug">
									<div class="font-semibold text-popover-foreground">{m.label}</div>
									<div class="mt-0.5 text-muted-foreground">{m.hint}</div>
								</div>
							</Tooltip.Content>
						</Tooltip.Root>
					{/each}
				</div>
			</Tooltip.Provider>
		</div>
	</div>
{:else}
	<!-- Standard (non-cinema) shell. Matches the cinema branch's
	     transitions in reverse: fades out as cinema fades in, and
	     scales slightly down as it leaves so the eye reads "this is
	     receding while the cinema canvas grows". -->
	<div
		class="relative min-h-screen text-foreground"
		in:fade={{ duration: 420, easing: quintOut }}
		out:scale={{ start: 0.98, duration: 320, easing: quintOut, opacity: 0.5 }}
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

		<!-- Sticky single-row top bar. Three zones — brand (left), mode
		     switcher (center), share + dashboard CTA (right). Glass-blur
		     bg so it reads cleanly over the gradient backdrop without
		     hiding the page. Width follows the active mode's max-w so
		     elements never look stranded on wide screens. -->
		<header class="sticky top-0 z-30 border-b border-border-low/30 bg-background/70 backdrop-blur-xl">
			<div class={cn(
				"mx-auto flex items-center gap-3 px-5 py-3 sm:px-8 transition-all duration-500 ease-out",
				wrapMaxWidth,
			)}>
				<a
					href="/"
					class="group/logo flex shrink-0 items-center gap-2.5"
					aria-label="Recast — home"
				>
					<span
						class="grid size-9 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
					>
						<Logo size="22" color="transparent" fill="currentColor" />
					</span>
					<span class="text-base font-semibold tracking-tight text-foreground max-sm:hidden">
						Recast
					</span>
				</a>

				<!-- Mode switcher. Segmented control with brand accent for the
				     active mode; tooltip hint surfaces the mode purpose so the
				     viewer can pick the right one without trial-and-error. -->
				<Tooltip.Provider delayDuration={250}>
					<div
						role="tablist"
						aria-label="View mode"
						class="mx-auto flex items-center gap-0.5 rounded-xl border border-border-low/40 bg-foreground/3 p-1"
					>
						{#each MODES as m (m.id)}
							<Tooltip.Root>
								<Tooltip.Trigger
									role="tab"
									aria-selected={mode === m.id}
									aria-label="{m.label} — {m.hint}"
									onclick={() => (mode = m.id)}
									class={cn(
										"group/mode relative grid size-8 place-items-center rounded-lg text-muted-foreground transition-all duration-200",
										"hover:bg-foreground/8 hover:text-foreground",
										mode === m.id && "bg-primary/15 text-primary",
									)}
								>
									<m.icon class="size-3.5" />
								</Tooltip.Trigger>
								<Tooltip.Content sideOffset={8} class="max-w-56 bg-popover">
									<div class="text-[11px] leading-snug">
										<div class="font-semibold text-popover-foreground">{m.label}</div>
										<div class="mt-0.5 text-muted-foreground">{m.hint}</div>
									</div>
								</Tooltip.Content>
							</Tooltip.Root>
						{/each}
					</div>
				</Tooltip.Provider>

				<div class="flex shrink-0 items-center gap-2">
					<!-- Theme toggle — single source of truth for both signed-in
					     and signed-out viewers. Icon-only ghost button, swaps
					     between Sun and Moon with a quick fly transition. -->
					<Tooltip.Provider delayDuration={300}>
						<Tooltip.Root>
							<Tooltip.Trigger
								onclick={toggleMode}
								aria-label={themeMode.current === "dark" ? "Switch to light mode" : "Switch to dark mode"}
								class="group/theme grid size-9 shrink-0 place-items-center rounded-xl border border-border/40 bg-foreground/5 text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground focus-visible:outline-2 focus-visible:outline-primary"
							>
								<span class="relative grid size-3.5 place-items-center">
									{#if themeMode.current === "dark"}
										<span
											class="absolute"
											in:fly={{ y: 4, duration: 180, easing: cubicOut }}
											out:fade={{ duration: 120 }}
										>
											<Sun class="size-3.5" />
										</span>
									{:else}
										<span
											class="absolute"
											in:fly={{ y: -4, duration: 180, easing: cubicOut }}
											out:fade={{ duration: 120 }}
										>
											<Moon class="size-3.5" />
										</span>
									{/if}
								</span>
							</Tooltip.Trigger>
							<Tooltip.Content sideOffset={8}>
								<span class="text-[11px]">
									{themeMode.current === "dark" ? "Light mode" : "Dark mode"}
								</span>
							</Tooltip.Content>
						</Tooltip.Root>
					</Tooltip.Provider>

					<!-- Share dropdown — promoted to primary styling (brand
					     lime fill, dark text) because it IS the headline action
					     on this page. Items are static; "Copy at M:SS" relabels
					     live with the playhead so users see what they're about
					     to copy before clicking. -->
					<DropdownMenu.Root>
						<DropdownMenu.Trigger
							class="group/share inline-flex h-9 items-center gap-1.5 rounded-xl border border-transparent bg-primary px-3 text-xs font-semibold text-primary-foreground shadow-craft-sm transition-all hover:scale-[1.02] hover:bg-primary/95 active:scale-[0.98] focus-visible:outline-2 focus-visible:outline-primary"
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
								<span class="ml-auto font-mono text-[10px] tabular-nums text-foreground">
									{formatTime(currentTime)}
								</span>
							</DropdownMenu.Item>
							<DropdownMenu.Item onclick={copyEmbedCode} class="gap-2.5">
								<Code class="size-3.5 text-muted-foreground" />
								Copy embed code
							</DropdownMenu.Item>
							<DropdownMenu.Separator />
							<DropdownMenu.Item
								onclick={() => browser && window.open(recast!.src, "_blank")}
								class="gap-2.5"
							>
								<Download class="size-3.5 text-muted-foreground" />
								Download original
							</DropdownMenu.Item>

							{#if canManage}
								<!-- Access controls. Only the owner or a global
								     admin sees this section. Optimistic write
								     into local `visibility` state; the API call
								     toasts loading/success/error and rolls back
								     on failure. -->
								<DropdownMenu.Separator />
								<DropdownMenu.Label class="text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
									Who can view
								</DropdownMenu.Label>
								{#each [
									{ id: "public" as const, label: "Anyone with the link", icon: Globe, hint: "Public" },
									{ id: "team" as const, label: "Only my team", icon: Users, hint: "Signed-in teammates" },
									{ id: "private" as const, label: "Only me", icon: Lock, hint: "Private" },
								] as opt (opt.id)}
									{@const active = visibility === opt.id}
									<DropdownMenu.Item
										disabled={updatingVisibility}
										onSelect={(e) => {
											// Keep the menu open so the user can see the
											// updated radio state before deciding to close.
											e.preventDefault();
											updateVisibility(opt.id);
										}}
										class="gap-2.5"
									>
										<opt.icon class={cn("size-3.5", active ? "text-primary" : "text-muted-foreground")} />
										<div class="flex-1 min-w-0">
											<div class={cn("text-xs", active && "font-medium text-foreground")}>
												{opt.label}
											</div>
										</div>
										{#if active}
											<Check class="size-3.5 text-primary" />
										{/if}
									</DropdownMenu.Item>
								{/each}
							{/if}
						</DropdownMenu.Content>
					</DropdownMenu.Root>

					{#if viewer}
						<!-- Profile dropdown — single circular avatar instead of a
						     wide Dashboard button. Avatar uses the user's `image`
						     when present; the fallback tile reuses the brand
						     logo's recipe (foreground bg + background ink) so it
						     reads as part of the brand, not a random tinted
						     thumbnail. The dropdown header repeats the identity
						     so a hover tooltip would be redundant. -->
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								aria-label="Account menu — {viewer.name || viewer.email}"
								class="grid size-9 shrink-0 place-items-center overflow-hidden rounded-full bg-foreground text-[11px] font-bold text-background ring-1 ring-border/40 transition-transform hover:scale-105 focus-visible:outline-2 focus-visible:outline-primary"
							>
								{#if viewer.image}
									<img
										src={viewer.image}
										alt=""
										referrerpolicy="no-referrer"
										class="size-full object-cover"
									/>
								{:else}
									{initials(viewer.name, viewer.email)}
								{/if}
							</DropdownMenu.Trigger>
							<DropdownMenu.Content align="end" sideOffset={8} class="w-64">
								<!-- Header — identity at-a-glance so the user can
								     confirm which account they're on before they
								     navigate or sign out. -->
								<div class="flex items-center gap-2.5 px-2 py-2.5">
									<span
										class="grid size-9 shrink-0 place-items-center overflow-hidden rounded-full bg-foreground text-[12px] font-bold text-background ring-1 ring-border/40"
									>
										{#if viewer.image}
											<img
												src={viewer.image}
												alt=""
												referrerpolicy="no-referrer"
												class="size-full object-cover"
											/>
										{:else}
											{initials(viewer.name, viewer.email)}
										{/if}
									</span>
									<div class="min-w-0 flex-1">
										<div class="truncate text-sm font-semibold text-foreground">
											{viewer.name || "Recast user"}
										</div>
										<div class="truncate font-mono text-[10px] text-muted-foreground">
											{viewer.email}
										</div>
									</div>
								</div>
								<DropdownMenu.Separator />
								<DropdownMenu.Item onclick={() => goto("/dashboard")} class="gap-2.5">
									<LayoutDashboard class="size-3.5 text-muted-foreground" />
									Dashboard
								</DropdownMenu.Item>
								<DropdownMenu.Item
									onclick={() => goto("/dashboard/recasts")}
									class="gap-2.5"
								>
									<Film class="size-3.5 text-muted-foreground" />
									My recasts
								</DropdownMenu.Item>
								<DropdownMenu.Item
									onclick={() => goto("/dashboard/settings/profile")}
									class="gap-2.5"
								>
									<Settings class="size-3.5 text-muted-foreground" />
									Account settings
								</DropdownMenu.Item>
								<DropdownMenu.Separator />
								<DropdownMenu.Item
									onclick={signOut}
									class="gap-2.5 text-destructive focus:bg-destructive/10 focus:text-destructive"
								>
									<LogOut class="size-3.5" />
									Sign out
								</DropdownMenu.Item>
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{:else}
						<!-- Unauthed viewer — small subtle CTA, not a heavy
						     "create account" pitch. The Share button already
						     anchors the primary action. -->
						<Button href="/login" size="sm" variant="outline" class="gap-1.5">
							<User class="size-3.5" />
							<span class="max-sm:hidden">Sign in</span>
						</Button>
					{/if}
				</div>
			</div>
		</header>

		<main
			class={cn(
				"mx-auto px-5 pb-16 pt-7 sm:px-8 transition-all duration-500 ease-out",
				wrapMaxWidth,
			)}
		>
			<!-- Title block — height-stable across modes so the video
			     position doesn't jump on switch. Shared-by meta is a small
			     inline chip below the title rather than a separate footer
			     row, so the viewer reads context next to the title. -->
			<div class="mb-5">
				<h1 class="text-balance text-2xl font-semibold leading-tight tracking-tight sm:text-3xl">
					{recast!.title}
				</h1>
				<div class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
					<span>
						Shared by <span class="font-medium text-foreground">{recast!.sharedBy}</span>
					</span>
					<span aria-hidden="true">·</span>
					<span>{new Date(recast!.sharedAt).toLocaleDateString(undefined, { month: "short", day: "numeric" })}</span>
				</div>
				<p class="mt-3 max-w-2xl text-sm leading-relaxed text-muted-foreground">
					{recast!.description}
				</p>
			</div>

			<!-- Main split — video column + (optional) rail. Flex with
			     gap morphs smoothly when the rail enters/exits. Tailwind's
			     `transition-all` handles the width change; Svelte's
			     `transition:slide` handles the rail itself. -->
			<div class="flex flex-col gap-6 lg:flex-row">
				<section class="min-w-0 flex-1 transition-all duration-500 ease-out">
					<div class="glass-card overflow-hidden rounded-2xl shadow-craft-xl">
						<RecastPlayer
							bind:api
							src={recast!.src}
							poster={recast!.poster}
							title={recast!.title}
							onengagement={(e) => {
								if (e.type === "view-start") isPlaying = true;
								if (e.type === "progress") {
									currentTime = e.currentTime;
									watchedPct = e.percent;
									isPlaying = true;
								}
								if (e.type === "ended") {
									watchedPct = 100;
									isPlaying = false;
								}
							}}
						/>
					</div>

					<!-- Engagement stats live on the owner's dashboard, not on
					     the public viewer's page. The player chrome already
					     surfaces current time + duration; that's enough
					     context here. Stays a $derived value so the share
					     dropdown's "Copy at M:SS" label and inline transcript
					     highlight keep updating. -->
				</section>

				<!-- Side rail — single "Review" surface with Transcript + Comments
				     as tabs. Rail itself slides in from the right when review
				     mode activates; the tab swap inside slides horizontally
				     so the viewer reads it as one panel laterally flipping
				     between two related views (not two disconnected modes). -->
				{#if showRail}
					<aside
						class="w-full lg:w-96 lg:shrink-0"
						in:slide={{ axis: "x", duration: 320, easing: cubicOut }}
						out:slide={{ axis: "x", duration: 220, easing: cubicOut }}
					>
						<div class="glass-card flex h-full max-h-160 flex-col overflow-hidden rounded-2xl shadow-craft-lg">
							<!-- Tab header. Two-button segmented control; active
							     tab gets a primary-tinted underline + foreground
							     text so it reads as "selected" at a glance. The
							     count badge slides between tabs so the viewer can
							     see comment / cue volume without opening each tab. -->
							<header
								role="tablist"
								aria-label="Review panel"
								class="grid grid-cols-2 border-b border-border-low/50"
							>
								{#each [
									{ id: "transcript" as const, label: "Transcript", icon: ScrollText, count: transcript.length },
									{ id: "comments" as const, label: "Comments", icon: MessageSquare, count: comments.length },
								] as t (t.id)}
									{@const active = reviewTab === t.id}
									<button
										type="button"
										role="tab"
										aria-selected={active}
										onclick={() => (reviewTab = t.id)}
										class={cn(
											"group/tab relative flex items-center justify-center gap-2 px-4 py-3 text-sm font-medium transition-colors",
											active
												? "text-foreground"
												: "text-muted-foreground hover:text-foreground",
										)}
									>
										<t.icon class={cn("size-4", active && "text-primary")} />
										<span>{t.label}</span>
										<span class={cn(
											"rounded-md px-1.5 py-0.5 font-mono text-[10px] tabular-nums transition-colors",
											active ? "bg-primary/15 text-primary" : "bg-foreground/8 text-muted-foreground",
										)}>
											{t.count}
										</span>
										{#if active}
											<span
												class="absolute inset-x-3 bottom-0 h-0.5 rounded-full bg-primary"
												in:slide={{ axis: "x", duration: 220, easing: cubicOut }}
											></span>
										{/if}
									</button>
								{/each}
							</header>

							<!-- Tab body. Absolute positioning lets both tabs share
							     the same coordinate space during the slide so they
							     truly cross laterally (no jump while one mounts).
							     Direction-aware: comments enters from the right
							     and transcript from the left, matching the tab
							     order so the motion reads as "moving along the
							     row". -->
							<div class="relative flex-1 overflow-hidden">
								{#if reviewTab === "transcript"}
									<div
										class="absolute inset-0 flex flex-col"
										in:fly={{ x: -32, duration: 260, easing: cubicOut, opacity: 0 }}
										out:fly={{ x: -32, duration: 200, easing: cubicOut, opacity: 0 }}
									>
										<ol bind:this={transcriptListEl} class="flex-1 overflow-y-auto px-1.5 py-2">
											{#each transcript as cue (cue.start)}
												{@const active = activeCue?.start === cue.start}
												<li>
													<button
														type="button"
														data-cue-start={cue.start}
														onclick={() => jumpTo(cue.start)}
														class={cn(
															"group/cue flex w-full items-start gap-3 rounded-xl px-3 py-2.5 text-left transition-colors",
															active ? "bg-primary/12" : "hover:bg-foreground/5",
														)}
													>
														<span class={cn(
															"shrink-0 rounded-md px-1.5 py-0.5 font-mono text-[10px] tabular-nums",
															active
																? "bg-primary/20 text-primary"
																: "bg-foreground/8 text-muted-foreground",
														)}>
															{formatTime(cue.start)}
														</span>
														<p class={cn(
															"text-[13px] leading-relaxed",
															active ? "text-foreground" : "text-foreground/75",
														)}>
															{cue.text}
														</p>
													</button>
												</li>
											{/each}
										</ol>
									</div>
								{:else}
									<div
										class="absolute inset-0 flex flex-col"
										in:fly={{ x: 32, duration: 260, easing: cubicOut, opacity: 0 }}
										out:fly={{ x: 32, duration: 200, easing: cubicOut, opacity: 0 }}
									>
										<ul class="flex-1 overflow-y-auto px-1.5 py-2">
											{#each comments as c (c.id)}
												{@const within = Math.abs(c.atSeconds - smoothedTime.current) < 5}
												<li
													class={cn(
														"group/comment flex items-start gap-3 rounded-xl px-3 py-3 transition-colors",
														within ? "bg-primary/10" : "hover:bg-foreground/5",
													)}
												>
													<button
														type="button"
														onclick={() => jumpTo(c.atSeconds)}
														aria-label="Jump to {c.author}'s comment at {formatTime(c.atSeconds)}"
														class="grid size-8 shrink-0 place-items-center rounded-full text-[11px] font-bold text-white transition-transform hover:scale-105"
														style="background: hsl({c.avatarHue} 60% 45%);"
													>
														{c.author[0]}
													</button>
													<div class="min-w-0 flex-1">
														<button
															type="button"
															onclick={() => jumpTo(c.atSeconds)}
															class="-ml-1 flex items-center gap-2 rounded-md px-1 py-0.5 text-left transition-colors hover:bg-foreground/5"
														>
															<span class="text-sm font-semibold">{c.author}</span>
															<span class={cn(
																"font-mono text-[10px] tabular-nums",
																within ? "text-primary" : "text-muted-foreground",
															)}>
																{formatTime(c.atSeconds)}
															</span>
														</button>
														<p class="mt-0.5 text-[12.5px] leading-relaxed text-foreground/85">
															{#each parseCommentText(c.text) as seg, i (i)}
																{#if seg.kind === "text"}<!--
																-->{seg.text}<!--
															-->{:else if seg.kind === "timestamp"}<!--
																--><button
																	type="button"
																	onclick={() => jumpTo(seg.seconds)}
																	class="mx-px inline-flex -translate-y-px items-center gap-1 px-1.5 py-0 align-middle text-[10.5px] font-medium text-primary transition-colors hover:underline"
																>
																	{formatTime(seg.seconds)}
																</button><!--
															-->{:else if seg.kind === "mention"}<!--
																--><span class="mx-px rounded-md bg-foreground/10 px-1 font-medium text-foreground">@{seg.name}</span><!--
															-->{/if}
															{/each}
														</p>
													</div>
												</li>
											{/each}
										</ul>
										<footer class="border-t border-border-low/50 px-4 py-3">
											<div class="flex items-center gap-2">
												<input
													bind:this={inputEl}
													bind:value={draftText}
													type="text"
													placeholder="Comment at {formatTime(currentTime)}…"
													onkeydown={(e) => {
														if (e.key === "Enter" && !e.shiftKey) {
															e.preventDefault();
															postComment();
														}
													}}
													class="flex-1 rounded-lg border border-border-low/70 bg-background/80 px-3 py-2 text-xs text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
												/>
												<Tooltip.Provider delayDuration={250}>
													<Tooltip.Root>
														<Tooltip.Trigger
															onclick={insertCurrentTimestamp}
															aria-label="Insert current timestamp"
															class="grid size-9 place-items-center rounded-lg border border-border-low/70 bg-background/80 text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
														>
															<Clock class="size-3.5" />
														</Tooltip.Trigger>
														<Tooltip.Content sideOffset={8}>
															<span class="text-[11px]">
																Insert <span class="font-mono">[{formatTime(currentTime)}]</span>
															</span>
														</Tooltip.Content>
													</Tooltip.Root>
												</Tooltip.Provider>
												<Button
													size="sm"
													class="gap-1.5"
													disabled={!draftText.trim()}
													onclick={postComment}
												>
													Post
													<ArrowRight class="size-3.5" />
												</Button>
											</div>
											<p class="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10px] text-muted-foreground">
												<span class="inline-flex items-center gap-1">
													<Clock class="size-2.5" />
													<span class="font-mono">[m:ss]</span> jumps to a time
												</span>
												<span aria-hidden="true">·</span>
												<span class="inline-flex items-center gap-1">
													<AtSign class="size-2.5" />
													<span class="font-mono">@name</span> mentions a teammate
												</span>
											</p>
										</footer>
									</div>
								{/if}
							</div>
						</div>
					</aside>
				{/if}
			</div>

			<!-- Bottom footer removed — every action it used to host
			     (copy link / copy at time / download) is now one click
			     away in the top-bar Share dropdown. The page ends with
			     content, not chrome. -->
		</main>
	</div>
{/if}
{/if}
