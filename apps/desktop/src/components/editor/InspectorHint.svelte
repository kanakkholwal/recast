<script lang="ts">
	import { CircleHelp } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import * as Tooltip from "@recast/ui/tooltip";

	interface Props {
		content: string;
	}

	let { content }: Props = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		<Button
			variant="ghost"
			size="xs"
			class="inline-flex p-0 shrink-0 items-center justify-center rounded-full text-muted-foreground/60 hover:text-foreground"
			aria-label="More information"
		>
			<CircleHelp size={11} />
		</Button>
	</Tooltip.Trigger>
	<Tooltip.Content class="max-w-56 text-[11px] leading-relaxed">
		{content}
	</Tooltip.Content>
</Tooltip.Root>
