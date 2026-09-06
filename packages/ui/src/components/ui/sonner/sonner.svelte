<script lang="ts">
import { AlertCircle, AlertTriangle, Check, Info, LoaderCircle, X } from "@recast/icons";
import { mode } from "mode-watcher";
import { Toaster as Sonner, type ToasterProps as SonnerProps } from "svelte-sonner";

let { ...restProps }: SonnerProps = $props();
</script>

<!--
  Recast desktop/web Sonner theming.

  Visual contract: each toast is a 320px-wide card that *visually matches*
  the bottom-right corner notifications (auto-updater, what's-new) so the
  app has a single notification language. Same border, same shadow, same
  icon-badge geometry — variant is conveyed only by the badge tint.

  Position is bottom-right by default; consumers can still override via
  the `<Toaster position="...">` prop. Sonner's stack grows upward from
  the bottom, so toasts naturally pile on top of any persistent corner
  notification without forcing a layout coordination layer.

  Icons are @recast/icons only (the rest of the app is @recast/icons-only by design
  rule). Sonner renders our snippet inside its `[data-icon]` element, so
  `classes.icon` styles the *badge* and the snippet just supplies the
  glyph that sits inside it.
-->
<Sonner
	theme={mode.current}
	position="bottom-right"
	offset={16}
	mobileOffset={16}
	closeButton
	gap={8}
	class="toaster group"
	style="
    --normal-bg: var(--color-card);
    --normal-text: var(--color-foreground);
    --normal-border: var(--color-border);

    --success-bg: var(--color-card);
    --success-text: var(--color-foreground);
    --success-border: var(--color-border);

    --error-bg: var(--color-card);
    --error-text: var(--color-foreground);
    --error-border: var(--color-border);

    --warning-bg: var(--color-card);
    --warning-text: var(--color-foreground);
    --warning-border: var(--color-border);

    --info-bg: var(--color-card);
    --info-text: var(--color-foreground);
    --info-border: var(--color-border);

    /* Sonner's default close button floats half-outside the top-left, so override every var that positions it to sit inset at top-right. */
    --toast-close-button-start: unset;
    --toast-close-button-end: 0;
    --toast-close-button-transform: translate(-8px, 8px);
  "
	toastOptions={{
		classes: {
			toast:
				"w-[320px]! rounded-xl! border! border-border/70! bg-card/95! backdrop-blur-xl! shadow-lg! ring-1! ring-foreground/5! p-3! gap-3!",
			content: "gap-0.5!",
			title:
				"text-[13px]! font-semibold! leading-tight! text-foreground! tracking-[-0.006em]!",
			description: "text-[11.5px]! text-muted-foreground! leading-relaxed!",
			icon:
				"size-7! shrink-0! flex! items-center! justify-center! rounded-full! bg-foreground/5! text-muted-foreground! m-0!",
			closeButton:
				"size-5! rounded-md! border-0! bg-transparent! text-muted-foreground/70! hover:bg-foreground/5! hover:text-foreground!",
			actionButton: "text-[11px]! font-semibold!",
			cancelButton: "text-[11px]! text-muted-foreground!",
			loading: "[&_[data-icon]]:bg-primary/10! [&_[data-icon]]:text-primary!",
			success:
				"[&_[data-icon]]:bg-success/10! [&_[data-icon]]:text-success!",
			error:
				"[&_[data-icon]]:bg-destructive/10! [&_[data-icon]]:text-destructive!",
			warning:
				"[&_[data-icon]]:bg-warning/10! [&_[data-icon]]:text-warning!",
			info:
				"[&_[data-icon]]:bg-info/10! [&_[data-icon]]:text-info!",
		},
	}}
	{...restProps}
>
	{#snippet loadingIcon()}
		<LoaderCircle class="size-3.5 animate-spin" />
	{/snippet}
	{#snippet successIcon()}
		<Check class="size-3.5" />
	{/snippet}
	{#snippet errorIcon()}
		<AlertCircle class="size-3.5" />
	{/snippet}
	{#snippet infoIcon()}
		<Info class="size-3.5" />
	{/snippet}
	{#snippet warningIcon()}
		<AlertTriangle class="size-3.5" />
	{/snippet}
	{#snippet closeIcon()}
		<X class="size-3" />
	{/snippet}
</Sonner>
