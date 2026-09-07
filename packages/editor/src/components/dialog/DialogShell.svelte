<script lang="ts">
/**
 * The app's one dialog chrome: a flush header (neutral rounded-xl icon chip +
 * title + subtitle, no divider) over a scrolling body and a borderless footer,
 * matching the settings/home/editor card language. Every modal routes through
 * it so they read as one object; the surface tokens live in `dialog.styles`.
 */
import type { IconComponent } from "@recast/icons";
import * as Dialog from "@recast/ui/dialog";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import { DIALOG_BODY, DIALOG_FOOTER, DIALOG_HEADER, DIALOG_SURFACE } from "./dialog.styles";

type Tone = "default" | "destructive" | "success" | "muted";

interface Props {
	open: boolean;
	title: string;
	/** Second line under the title. Usually the file being acted on. */
	subtitle?: string;
	icon?: IconComponent;
	/** Tints the icon chip. Never the only signal — the copy says it too. */
	tone?: Tone;
	/** Caps the dialog width. Must be `sm:`-prefixed to beat Dialog.Content's own `sm:max-w-sm`. */
	widthClass?: string;
	/** Extra classes for the scrolling body. `max-h-none` lets media size itself. */
	bodyClass?: string;
	onOpenChange: (open: boolean) => void;
	children: Snippet;
	footer?: Snippet;
}

let {
	open = $bindable(false),
	title,
	subtitle,
	icon: Icon,
	tone = "default",
	widthClass = "sm:max-w-sm",
	bodyClass = "",
	onOpenChange,
	children,
	footer,
}: Props = $props();

const TONE_CHIP: Record<Tone, string> = {
	default: "bg-muted/60 text-foreground ring-1 ring-inset ring-border/50",
	destructive: "bg-destructive/10 text-destructive ring-1 ring-inset ring-destructive/20",
	success: "bg-success/10 text-success ring-1 ring-inset ring-success/20",
	muted: "bg-muted text-muted-foreground ring-1 ring-inset ring-border/50",
};
</script>

<Dialog.Root
	bind:open
	onOpenChange={(v) => {
		open = v;
		onOpenChange(v);
	}}
>
	<Dialog.Content
		showCloseButton={false}
		class={cn("block! gap-0!", DIALOG_SURFACE, widthClass)}
	>
		<Dialog.Header class={cn("flex-row items-center gap-3", DIALOG_HEADER)}>
			{#if Icon}
				<span
					class={cn(
						"grid size-9 shrink-0 place-items-center rounded-xl",
						TONE_CHIP[tone],
					)}
					aria-hidden="true"
				>
					<Icon class="size-4" />
				</span>
			{/if}
			<span class="flex min-w-0 flex-1 flex-col gap-0.5">
				<Dialog.Title class="text-[15px] leading-tight font-semibold tracking-tight text-balance text-foreground">
					{title}
				</Dialog.Title>
				<!-- Always rendered: bits-ui warns when a dialog has no description,
				     and an empty one keeps the header height stable between states. -->
				<Dialog.Description
					class={cn("truncate text-[11.5px] leading-relaxed text-muted-foreground", !subtitle && "sr-only")}
				>
					{subtitle ?? title}
				</Dialog.Description>
			</span>
		</Dialog.Header>

		<div class={cn(DIALOG_BODY, bodyClass)}>
			{@render children()}
		</div>

		{#if footer}
			<footer class={DIALOG_FOOTER}>
				{@render footer()}
			</footer>
		{/if}
	</Dialog.Content>
</Dialog.Root>
