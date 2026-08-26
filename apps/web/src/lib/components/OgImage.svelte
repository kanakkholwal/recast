<script lang="ts">
type Props = {
	title: string;
	description?: string;
	eyebrow?: string;
};

let { title, description = "", eyebrow = "" }: Props = $props();
</script>

<svelte:options css="injected" />

<div class="og">
	<div class="og-grid"></div>

	<div class="og-head">
		<div class="og-brand">
			<div class="og-mark">
				<span class="og-bar"></span>
				<span class="og-bar"></span>
				<span class="og-bar"></span>
			</div>
			<span class="og-wordmark">Recast</span>
		</div>

		{#if eyebrow}
			<div class="og-chip">{eyebrow}</div>
		{/if}
	</div>

	<div class="og-body">
		<h1 class="og-title">{title}</h1>
		{#if description}
			<p class="og-desc">{description}</p>
		{/if}
	</div>

	<div class="og-foot">
		<span class="og-url">recast.li</span>
		<span class="og-tag">
			<span>Record</span>
			{@render arrow()}
			<span>Polish</span>
			{@render arrow()}
			<span>Share</span>
		</span>
	</div>
</div>

<!--
	Inline SVG (not a "→" glyph): the OG image is rasterised by takumi's
	WebAssembly renderer in production, which has no system-font fallback, and
	Geist's latin subset has no U+2192 — a text arrow renders as tofu. takumi
	serialises an inline <svg> and hands the markup to resvg, so this is
	glyph-independent. xmlns is required for resvg to parse the standalone SVG.
-->
{#snippet arrow()}
	<svg
		class="og-arrow"
		xmlns="http://www.w3.org/2000/svg"
		width="24"
		height="24"
		viewBox="0 0 24 24"
		fill="none"
		stroke="#b0eb4e"
		stroke-width="2.5"
		stroke-linecap="round"
		stroke-linejoin="round"
	>
		<path d="M4 12h15" />
		<path d="M13 6l6 6-6 6" />
	</svg>
{/snippet}

<style>
	.og {
		position: relative;
		width: 1200px;
		height: 630px;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		padding: 72px 80px;
		background: #0a0a0a;
		color: #f5f5f5;
		font-family:
			"Geist", "Inter", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto,
			"Helvetica Neue", Arial, sans-serif;
		overflow: hidden;
		box-sizing: border-box;
	}

	.og-grid {
		position: absolute;
		inset: 0;
		background-image:
			linear-gradient(to right, rgba(255, 255, 255, 0.035) 1px, transparent 1px),
			linear-gradient(to bottom, rgba(255, 255, 255, 0.035) 1px, transparent 1px);
		background-size: 64px 64px;
	}

	.og-head,
	.og-body,
	.og-foot {
		position: relative;
		display: flex;
	}

	.og-head {
		justify-content: space-between;
		align-items: center;
	}

	.og-brand {
		display: flex;
		align-items: center;
		gap: 18px;
	}

	.og-mark {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		width: 72px;
		height: 72px;
		border-radius: 50%;
		background: #ffffff;
		box-sizing: border-box;
	}

	.og-bar {
		display: block;
		width: 9px;
		height: 25px;
		border-radius: 5px;
		background: #0a0a0a;
	}

	.og-wordmark {
		font-size: 36px;
		font-weight: 600;
		letter-spacing: -0.02em;
		color: #fafafa;
	}

	.og-chip {
		display: inline-flex;
		align-items: center;
		padding: 10px 18px;
		font-size: 16px;
		font-weight: 600;
		letter-spacing: 0.18em;
		text-transform: uppercase;
		color: #d4f57a;
		background: rgba(176, 235, 78, 0.1);
		border: 1px solid rgba(176, 235, 78, 0.32);
		border-radius: 999px;
	}

	.og-body {
		flex-direction: column;
		gap: 28px;
		max-width: 1040px;
	}

	.og-title {
		font-size: 84px;
		font-weight: 600;
		line-height: 1.02;
		letter-spacing: -0.035em;
		color: #fafafa;
		margin: 0;
	}

	.og-desc {
		font-size: 30px;
		line-height: 1.35;
		letter-spacing: -0.01em;
		color: rgba(245, 245, 245, 0.62);
		font-weight: 450;
		margin: 0;
		max-width: 940px;
	}

	.og-foot {
		justify-content: space-between;
		align-items: center;
		padding-top: 24px;
		border-top: 1px solid rgba(255, 255, 255, 0.08);
		font-size: 20px;
		color: rgba(245, 245, 245, 0.65);
		font-weight: 500;
	}

	.og-url {
		color: #fafafa;
		font-weight: 600;
	}

	.og-tag {
		display: flex;
		align-items: center;
		gap: 12px;
		letter-spacing: 0.02em;
	}

	.og-arrow {
		display: block;
		width: 24px;
		height: 24px;
	}
</style>
