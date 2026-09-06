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
	<div class="og-glow"></div>

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
			<!-- The split pill from the hero: label, divider, then the site it points at. -->
			<div class="og-pill">
				<span class="og-pill-label">{eyebrow}</span>
				<span class="og-pill-tail">recast.li</span>
			</div>
		{/if}
	</div>

	<div class="og-body">
		<h1 class="og-title">{title}</h1>
		{#if description}
			<p class="og-desc">{description}</p>
		{/if}
	</div>

	<div class="og-foot">
		<span class="og-tag">
			<span>Record</span>
			{@render arrow()}
			<span>Polish</span>
			{@render arrow()}
			<span>Share</span>
		</span>
		<span class="og-meta">Free and open source</span>
	</div>
</div>

<!--
	Inline SVG (not a "→" glyph): the OG image is rasterised by takumi's
	WebAssembly renderer in production, which has no system-font fallback, and
	Satoshi's latin subset has no U+2192 — a text arrow renders as tofu. takumi
	serialises an inline <svg> and hands the markup to resvg, so this is
	glyph-independent. xmlns is required for resvg to parse the standalone SVG.
-->
{#snippet arrow()}
	<svg
		class="og-arrow"
		xmlns="http://www.w3.org/2000/svg"
		width="22"
		height="22"
		viewBox="0 0 24 24"
		fill="none"
		stroke="#6c9bfe"
		stroke-width="2.5"
		stroke-linecap="round"
		stroke-linejoin="round"
	>
		<path d="M4 12h15" />
		<path d="M13 6l6 6-6 6" />
	</svg>
{/snippet}

<style>
	/* Hex, not tokens: takumi rasterises this with no stylesheet, so these are packages/design's dark theme converted from oklch. */
	.og {
		position: relative;
		width: 1200px;
		height: 630px;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		padding: 72px 80px;
		background: #161717;
		color: #f0f0f0;
		font-family: "Inter", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
		overflow: hidden;
		box-sizing: border-box;
	}

	.og-grid {
		position: absolute;
		inset: 0;
		background-image:
			linear-gradient(to right, rgba(255, 255, 255, 0.03) 1px, transparent 1px),
			linear-gradient(to bottom, rgba(255, 255, 255, 0.03) 1px, transparent 1px);
		background-size: 64px 64px;
	}

	/* The landing hero's bloom behind the headline, anchored to the same corner. */
	.og-glow {
		position: absolute;
		top: -220px;
		left: -160px;
		width: 900px;
		height: 620px;
		background: radial-gradient(circle, rgba(108, 155, 254, 0.14) 0%, rgba(108, 155, 254, 0) 70%);
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
		width: 68px;
		height: 68px;
		border-radius: 20px;
		background: #f0f0f0;
		box-sizing: border-box;
	}

	.og-bar {
		display: block;
		width: 8px;
		height: 24px;
		border-radius: 5px;
		background: #161717;
	}

	.og-wordmark {
		font-family: "Satoshi", "Inter", sans-serif;
		font-size: 34px;
		font-weight: 600;
		letter-spacing: -0.02em;
		color: #f0f0f0;
	}

	.og-pill {
		display: flex;
		align-items: stretch;
		background: #1c1c1c;
		border: 1px solid #333333;
		border-radius: 999px;
		overflow: hidden;
		font-size: 17px;
		font-weight: 500;
	}

	.og-pill-label {
		display: flex;
		align-items: center;
		padding: 12px 16px 12px 22px;
		color: #f0f0f0;
	}

	.og-pill-tail {
		display: flex;
		align-items: center;
		padding: 12px 22px 12px 16px;
		border-left: 1px solid #333333;
		color: #8c8c8c;
	}

	.og-body {
		flex-direction: column;
		gap: 26px;
		max-width: 1040px;
	}

	.og-title {
		font-family: "Satoshi", "Inter", sans-serif;
		font-size: 84px;
		font-weight: 600;
		line-height: 1.04;
		letter-spacing: -0.035em;
		color: #f0f0f0;
		margin: 0;
	}

	.og-desc {
		font-size: 29px;
		line-height: 1.38;
		letter-spacing: -0.01em;
		color: #8c8c8c;
		font-weight: 450;
		margin: 0;
		max-width: 930px;
	}

	.og-foot {
		justify-content: space-between;
		align-items: center;
		padding-top: 26px;
		border-top: 1px solid #333333;
		font-size: 20px;
		font-weight: 500;
	}

	.og-tag {
		display: flex;
		align-items: center;
		gap: 12px;
		letter-spacing: 0.01em;
		color: #f0f0f0;
	}

	.og-meta {
		color: #8c8c8c;
	}

	.og-arrow {
		display: block;
		width: 22px;
		height: 22px;
	}
</style>
