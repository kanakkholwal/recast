<script lang="ts">
/**
 * The app's one dialog chrome: flush content, a bordered header carrying an
 * icon + title + subtitle, and a bordered footer on a muted bar.
 *
 * It exists because the dialogs had drifted into two dialects — the "craft"
 * one (Confirm, Rename, profiles: `p-0`, ring, bordered footer, `xs` buttons)
 * and the plain primitive one (the cloud dialogs: default padding,
 * `Dialog.Header`/`Footer`, `sm` buttons). Same product, two visual languages.
 */
import type { IconComponent } from "@recast/icons";
import * as Dialog from "@recast/ui/dialog";
import { DIALOG_BODY, DIALOG_FOOTER, DIALOG_HEADER, DIALOG_SURFACE } from "./dialog.styles";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";

type Tone = "default" | "destructive" | "muted";

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
	default: "border-primary/20 bg-primary/10 text-primary",
	destructive: "border-destructive/20 bg-destructive/10 text-destructive",
	muted: "border-border/50 bg-muted text-muted-foreground",
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
		<Dialog.Header class={cn("flex-row items-start gap-3", DIALOG_HEADER)}>
			{#if Icon}
				<span
					class={cn(
						"grid size-8 shrink-0 place-items-center rounded-lg border",
						TONE_CHIP[tone],
					)}
					aria-hidden="true"
				>
					<Icon class="size-3.5" />
				</span>
			{/if}
			<span class="flex min-w-0 flex-1 flex-col gap-0.5">
				<Dialog.Title class="text-[13px] font-semibold tracking-tight text-foreground">
					{title}
				</Dialog.Title>
				<!-- Always rendered: bits-ui warns when a dialog has no description,
				     and an empty one keeps the header height stable between states. -->
				<Dialog.Description
					class={cn("truncate text-[11px] leading-relaxed text-muted-foreground", !subtitle && "sr-only")}
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
