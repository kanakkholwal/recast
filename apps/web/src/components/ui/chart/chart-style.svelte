<script lang="ts">
import { type ChartConfig, THEMES } from "./chart-utils.js";

let { id, config }: { id: string; config: ChartConfig } = $props();

const colorConfig = $derived(
	config ? Object.entries(config).filter(([, entry]) => entry.theme || entry.color) : null,
);

const themeContents = $derived.by(() => {
	if (!colorConfig?.length) return;

	const blocks = [];
	for (const [themeName, prefix] of Object.entries(THEMES)) {
		let content = `${prefix} [data-chart=${id}] {\n`;
		const vars = colorConfig.map(([key, itemConfig]) => {
			const theme = themeName as keyof typeof itemConfig.theme;
			const value = itemConfig.theme?.[theme] || itemConfig.color;
			return value ? `\t--color-${key}: ${value};` : null;
		});

		content += `${vars.join("\n")}\n}`;

		blocks.push(content);
	}

	return blocks.join("\n");
});
</script>

{#if themeContents}
	{#key id}
		<svelte:element this={"style"}>
			{themeContents}
		</svelte:element>
	{/key}
{/if}
