<script lang="ts">
import { prefersReducedMotion } from "$lib/motion-core";
import { Layout, MonitorPlay, Play, Search } from "@recast/icons";
import { cn } from "@recast/ui/utils";

// Command-palette mock for Record. A single highlight slides between rows
// rather than each row swapping its own background: one moving object reads as
// keyboard navigation, three cross-fading backgrounds read as a flicker.
//
// The query line retypes itself to match the row being landed on, which is what
// ties the two halves of the mock together.
//
// Reduced motion pins it to the first row with the query already typed.
const options = [
	{ icon: MonitorPlay, label: "Record full screen", query: "full screen" },
	{ icon: Layout, label: "Record region", query: "region" },
	{ icon: Play, label: "Continue last project", query: "last project" },
];

const DWELL_MS = 2600;
const TYPE_MS = 55;

const reduced = $derived(prefersReducedMotion());
let active = $state(0);
let typed = $state("");

// Row pitch drives the highlight's offset. Kept as one constant so the
// translate and the row height can never drift apart.
const ROW = "2.375rem";

$effect(() => {
	if (reduced) {
		active = 0;
		typed = options[0].query;
		return;
	}

	let cancelled = false;
	let timer: ReturnType<typeof setTimeout>;

	async function run() {
		while (!cancelled) {
			const target = options[active].query;

			// Retype the query one character at a time.
			for (let i = 0; i <= target.length; i++) {
				if (cancelled) return;
				typed = target.slice(0, i);
				await new Promise<void>((r) => {
					timer = setTimeout(r, TYPE_MS);
				});
			}

			await new Promise<void>((r) => {
				timer = setTimeout(r, DWELL_MS);
			});
			if (cancelled) return;

			// Pause while the tab is hidden so it never animates off-screen.
			if (!document.hidden) active = (active + 1) % options.length;
		}
	}

	run();
	return () => {
		cancelled = true;
		clearTimeout(timer);
	};
});
</script>

<div class="p-5">
	<div class="rounded-xl border border-border-low bg-card p-3">
		<!-- Query line -->
		<div class="flex items-center gap-3 rounded-lg border border-border-low bg-background px-3 py-2.5">
			<Search class="size-4 shrink-0 text-muted-foreground" />
			<span class="min-w-0 truncate text-body-sm text-foreground">
				{#if typed}
					<span class="font-medium">{typed}</span>
				{:else}
					<span class="text-muted-foreground">Start a recording…</span>
				{/if}
				<span class="caret" aria-hidden="true"></span>
			</span>
			<kbd
				class="ml-auto shrink-0 rounded-md border border-border-low bg-card px-1.5 py-0.5 font-mono text-caption font-medium text-muted-foreground"
			>
				⌘ ⇧ R
			</kbd>
		</div>

		<!-- Options. The highlight is one absolutely-positioned element that
		     translates; the rows themselves never change background. -->
		<div class="relative mt-2" style={`--row:${ROW}`}>
			<div
				aria-hidden="true"
				class="absolute inset-x-0 top-0 rounded-md bg-paper transition-transform duration-300 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none"
				style={`height:var(--row); transform:translateY(calc(${active} * var(--row)))`}
			></div>

			{#each options as opt, i (opt.label)}
				{@const Icon = opt.icon}
				<div
					class="relative flex items-center gap-3 px-3 text-body-sm"
					style={`height:${ROW}`}
				>
					<Icon
						class={cn(
							"size-4 shrink-0 transition-colors duration-300",
							i === active ? "text-foreground" : "text-muted-foreground",
						)}
					/>
					<span
						class={cn(
							"font-medium transition-colors duration-300",
							i === active ? "text-foreground" : "text-muted-foreground",
						)}
					>
						{opt.label}
					</span>
					{#if i === active}
						<kbd
							class="ml-auto rounded border border-border-low bg-card px-1 py-0.5 font-mono text-caption text-muted-foreground"
						>
							↵
						</kbd>
					{/if}
				</div>
			{/each}
		</div>
	</div>
</div>

<style>
	.caret {
		display: inline-block;
		width: 1px;
		height: 0.9em;
		margin-left: 1px;
		vertical-align: text-bottom;
		background-color: currentColor;
		animation: blink 1.1s steps(1) infinite;
	}

	@keyframes blink {
		0%,
		49% {
			opacity: 1;
		}
		50%,
		100% {
			opacity: 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.caret {
			animation: none;
			opacity: 0;
		}
	}
</style>
