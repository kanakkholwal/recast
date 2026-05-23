<script lang="ts">
	import { browser } from "$app/environment";
	import { page } from "$app/state";
	import Logo from "$lib/logo.svelte";
	import {
	  ArrowRight,
	  AtSign,
	  Clock,
	  Download,
	  Film,
	  Link2,
	  MessageSquare,
	  Pause,
	  Play,
	  RectangleHorizontal,
	  ScrollText,
	  Square,
	} from "@lucide/svelte";
	import { RecastPlayer, type RecastPlayerApi } from "@recast/player";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import * as Tooltip from "@recast/ui/tooltip";
	import { cn } from "@recast/ui/utils";
	import { onMount, untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { Tween } from "svelte/motion";
	import { fade, fly, slide } from "svelte/transition";

	let { data } = $props();

	/**
	 * Five viewing modes — picked to cover the different reasons someone
	 * lands on a share link:
	 *
	 *   • focus      — video as the message; minimal chrome around it
	 *   • theater    — wide video with description right below, YouTube-ish
	 *   • comments   — discussion is the value; timestamped comment rail
	 *   • transcript — content-driven; clickable cue list auto-scrolls
	 *   • cinema     — full-bleed, controls auto-hide, distraction-free
	 *
	 * Mode persists per-viewer in localStorage and can be pinned via
	 * `?view=` for share-link presets ("send this to a stakeholder in
	 * cinema mode").
	 */
	type Mode = "focus" | "theater" | "comments" | "transcript" | "cinema";

	const MODES: { id: Mode; label: string; icon: typeof Square; hint: string }[] = [
		{ id: "focus", label: "Focus", icon: Square, hint: "Centered, no extras" },
		{ id: "theater", label: "Theater", icon: RectangleHorizontal, hint: "Wide video, details below" },
		{ id: "comments", label: "Comments", icon: MessageSquare, hint: "Timestamped discussion rail" },
		{ id: "transcript", label: "Transcript", icon: ScrollText, hint: "Clickable cue list" },
		{ id: "cinema", label: "Cinema", icon: Film, hint: "Full-bleed, distraction-free" },
	];

	function readInitialMode(): Mode {
		const urlMode = page.url.searchParams.get("view") as Mode | null;
		if (urlMode && MODES.some((m) => m.id === urlMode)) return urlMode;
		return "theater";
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
	const initialSeekSeconds = untrack(() => parseTimeParam(page.url.searchParams.get("t")));

	onMount(() => {
		// Restore persisted mode unless the URL pinned one.
		if (!page.url.searchParams.get("view") && browser) {
			const stored = localStorage.getItem("recast.share.mode") as Mode | null;
			if (stored && MODES.some((m) => m.id === stored)) mode = stored;
		}
	});

	$effect(() => {
		if (!browser) return;
		// URL is the source of truth — every mode change updates both the
		// query string (via replaceState so the back button skips noise)
		// and localStorage (so a viewer's last-used mode survives a hard
		// refresh on a URL without `?view=`). `theater` is the default and
		// stays out of the URL to keep clean links clean.
		localStorage.setItem("recast.share.mode", mode);
		const params = new URLSearchParams(window.location.search);
		if (mode === "theater") params.delete("view");
		else params.set("view", mode);
		const search = params.toString();
		const newUrl = `${window.location.pathname}${search ? `?${search}` : ""}`;
		window.history.replaceState({}, "", newUrl);
	});

	let api = $state<RecastPlayerApi | null>(null);
	let initialSeekApplied = $state(false);

	// Apply the URL-supplied `?t=` seek the moment the player API
	// publishes. The seek translates to setting `videoEl.currentTime`,
	// which the browser queues until metadata is loaded — so this works
	// whether the player is already ready or still spinning up.
	$effect(() => {
		if (!api || initialSeekApplied || initialSeekSeconds <= 0) return;
		api.seek(initialSeekSeconds);
		initialSeekApplied = true;
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

	async function copyLinkAtCurrentTime() {
		if (!browser) return;
		const url = new URL(window.location.href);
		const t = compactTime(currentTime);
		if (t) url.searchParams.set("t", t);
		else url.searchParams.delete("t");
		try {
			await navigator.clipboard.writeText(url.toString());
			toast.success(
				t ? `Link copied at ${formatTime(currentTime)}.` : "Link copied.",
			);
		} catch {
			toast.error("Couldn't copy to clipboard.");
		}
	}

	// Layout math driven by the active mode. We do this in JS so the
	// values can be tween'd / animated; CSS Grid can't yet smoothly
	// interpolate template changes across browsers.
	const isFullBleed = $derived(mode === "cinema");
	const showRail = $derived(mode === "comments" || mode === "transcript");
	const wrapMaxWidth = $derived(
		mode === "focus" ? "max-w-3xl" :
		mode === "theater" ? "max-w-5xl" :
		mode === "comments" || mode === "transcript" ? "max-w-7xl" :
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
	<title>{data.recast.title} — Recast</title>
	<meta name="description" content={data.recast.description} />
	<meta name="robots" content="noindex" />
</svelte:head>

<!-- Cinema mode renders a completely different shell: full-bleed black
     canvas, controls auto-hide, no chrome around the video. Other modes
     share the standard share-page chrome. -->
{#if isFullBleed}
	<div
		class="fixed inset-0 z-50 grid place-items-center bg-black"
		in:fade={{ duration: 280, easing: cubicOut }}
		out:fade={{ duration: 180, easing: cubicOut }}
	>
		<div class="relative w-full h-full grid place-items-center">
			<div class="w-full max-w-[100vw] max-h-[100vh] aspect-video">
				<RecastPlayer
					bind:api
					src={data.recast.src}
					poster={data.recast.poster}
					title={data.recast.title}
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
			<!-- Top-right exit chip — same switcher pattern, restricted to
			     a single "leave cinema" affordance so the canvas stays
			     uncluttered. -->
			<Tooltip.Provider delayDuration={250}>
				<div
					class="absolute right-4 top-4 flex items-center gap-1.5"
					in:fly={{ y: -8, duration: 260, easing: cubicOut }}
				>
					{#each MODES as m (m.id)}
						<Tooltip.Root>
							<Tooltip.Trigger
								aria-label="{m.label} — {m.hint}"
								onclick={() => (mode = m.id)}
								class={cn(
									"grid size-8 place-items-center rounded-lg border border-border/30 bg-foreground/5 text-muted-foreground transition-all hover:bg-foreground/10 hover:text-foreground",
									mode === m.id && "border-primary/40 bg-primary/15 text-primary",
								)}
							>
								<m.icon class="size-3.5" />
							</Tooltip.Trigger>
							<Tooltip.Content sideOffset={8} class="max-w-56">
								<div class="text-[11px] leading-snug">
									<div class="font-semibold text-foreground">{m.label}</div>
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
	<div class="relative min-h-screen text-foreground">
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-0 -z-10"
			style="background: radial-gradient(ellipse 70% 50% at 50% 0%, color-mix(in srgb, var(--color-primary) 9%, transparent), transparent 72%);"
		></div>
		<div
			aria-hidden="true"
			class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 -z-10 opacity-30"
		></div>

		<header class={cn("mx-auto flex items-center justify-between gap-3 px-5 py-5 sm:px-8 sm:py-6 transition-all duration-500 ease-out", wrapMaxWidth)}>
			<a href="/" class="group/logo flex items-center gap-2.5" aria-label="Recast — home">
				<span
					class="grid size-9 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
				>
					<Logo size="22" color="transparent" fill="currentColor" />
				</span>
				<span class="text-lg font-semibold tracking-tight text-foreground">Recast</span>
			</a>

			<!-- Mode switcher. Segmented control with brand accent for the
			     active mode; tooltip hint surfaces the mode purpose so the
			     viewer can pick the right one without trial-and-error. -->
			<Tooltip.Provider delayDuration={250}>
				<div role="tablist" aria-label="View mode" class="glass-card flex items-center gap-0.5 rounded-xl p-1 shadow-craft-sm">
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

			<Button href="/" variant="outline" size="sm" class="gap-2 max-sm:hidden">
				Open Recast
				<ArrowRight class="size-3.5" />
			</Button>
		</header>

		<main
			class={cn(
				"mx-auto px-5 pb-16 sm:px-8 transition-all duration-500 ease-out",
				wrapMaxWidth,
			)}
		>
			<!-- Title block — height-stable across modes so the video
			     position doesn't jump on switch. -->
			<div class="mb-5">
				<h1 class="text-balance text-2xl font-semibold leading-tight tracking-tight sm:text-3xl">
					{data.recast.title}
				</h1>
				<p class="mt-1.5 max-w-2xl text-sm leading-relaxed text-muted-foreground">
					{data.recast.description}
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
							src={data.recast.src}
							poster={data.recast.poster}
							title={data.recast.title}
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

					<!-- Engagement strip — sits below the player in every
					     mode that includes it. Theater + focus get the
					     bigger stats grid; comments/transcript modes get a
					     condensed inline strip to leave room for the rail. -->
					{#if mode === "focus" || mode === "theater"}
						<div
							class="mt-5 grid gap-3 sm:grid-cols-3"
							in:fade={{ duration: 220, delay: 80 }}
							out:fade={{ duration: 140 }}
						>
							<div class="glass-card rounded-xl p-4">
								<div class="text-[10px] uppercase tracking-wider text-muted-foreground">Watched</div>
								<div class="mt-1 font-mono text-xl font-semibold tabular-nums">{watchedPct}%</div>
							</div>
							<div class="glass-card rounded-xl p-4">
								<div class="text-[10px] uppercase tracking-wider text-muted-foreground">Current</div>
								<div class="mt-1 font-mono text-xl font-semibold tabular-nums">{formatTime(currentTime)}</div>
							</div>
							<div class="glass-card rounded-xl p-4">
								<div class="text-[10px] uppercase tracking-wider text-muted-foreground">Comments nearby</div>
								<div class="mt-1 font-mono text-xl font-semibold tabular-nums">{activeCommentsCount}</div>
							</div>
						</div>
					{:else}
						<div
							class="mt-3 flex flex-wrap items-center gap-2.5 text-xs text-muted-foreground"
							in:fade={{ duration: 220, delay: 80 }}
							out:fade={{ duration: 140 }}
						>
							<button
								type="button"
								onclick={togglePlay}
								class="inline-flex items-center gap-1.5 rounded-full border border-border/40 bg-foreground/5 px-2.5 py-1 font-medium transition-colors hover:bg-foreground/10"
							>
								{#if isPlaying}<Pause class="size-3" />Pause{:else}<Play class="size-3" />Play{/if}
							</button>
							<span class="font-mono tabular-nums">{formatTime(currentTime)}</span>
							<span aria-hidden="true">·</span>
							<span><span class="font-mono tabular-nums">{watchedPct}%</span> watched</span>
						</div>
					{/if}
				</section>

				<!-- Side rail — comments OR transcript, slides in from the
				     right with a horizontal slide. `crossfade`-style would
				     be smoother for content swap, but `transition:slide`
				     keeps the implementation honest (rail truly leaves
				     the DOM when not needed → no offscreen rendering). -->
				{#if showRail}
					<aside
						class="w-full lg:w-96 lg:flex-shrink-0"
						in:slide={{ axis: "x", duration: 320, easing: cubicOut }}
						out:slide={{ axis: "x", duration: 220, easing: cubicOut }}
					>
						<div class="glass-card flex h-full max-h-[640px] flex-col overflow-hidden rounded-2xl shadow-craft-lg">
							{#if mode === "comments"}
								{#key "comments"}
									<header
										class="flex items-center justify-between border-b border-border-low/50 px-4 py-3"
										in:fade={{ duration: 240 }}
									>
										<div class="flex items-center gap-2 text-sm font-semibold">
											<MessageSquare class="size-4 text-primary" />
											Comments
										</div>
										<Badge variant="outline" class="text-[10px] font-mono">
											{comments.length}
										</Badge>
									</header>
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
													<!-- Header is the second click target — same anchor-jump
													     as the avatar, paired text + timestamp. Keeping them
													     separate from the body lets the body host its own
													     inline timestamp/mention chips without nesting buttons. -->
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
																class="mx-px inline-flex translate-y-[-1px] items-center gap-1 rounded-md bg-primary/15 px-1.5 py-0 align-middle font-mono text-[10.5px] font-semibold text-primary transition-colors hover:bg-primary/25"
															>
																<Clock class="size-3" />
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
										<!-- Tiny syntax helper — keeps the affordance discoverable
										     without taking the user to a docs page. -->
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
								{/key}
							{:else if mode === "transcript"}
								{#key "transcript"}
									<header
										class="flex items-center justify-between border-b border-border-low/50 px-4 py-3"
										in:fade={{ duration: 240 }}
									>
										<div class="flex items-center gap-2 text-sm font-semibold">
											<ScrollText class="size-4 text-primary" />
											Transcript
										</div>
										<Badge variant="outline" class="text-[10px] font-mono">
											{transcript.length} cues
										</Badge>
									</header>
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
								{/key}
							{/if}
						</div>
					</aside>
				{/if}
			</div>

			<footer class="mt-7 flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
				<span>
					Shared by <span class="font-medium text-foreground">{data.recast.sharedBy}</span>
				</span>
				<div class="flex items-center gap-4">
					<button
						type="button"
						onclick={copyLinkAtCurrentTime}
						title="Copy a link that opens at the current playhead"
						class="inline-flex items-center gap-1.5 font-semibold text-foreground transition-colors hover:text-primary"
					>
						<Link2 class="size-3.5" />
						Copy link at <span class="font-mono tabular-nums">{formatTime(currentTime)}</span>
					</button>
					<a
						href={data.recast.src}
						download
						class="inline-flex items-center gap-1.5 font-semibold text-foreground transition-colors hover:text-primary"
					>
						<Download class="size-3.5" />
						Download
					</a>
				</div>
			</footer>
		</main>
	</div>
{/if}
