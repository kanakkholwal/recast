<script lang="ts">
	import { mode } from "mode-watcher";
	import {
		AlertOctagon,
		AlertTriangle,
		CheckCircle2,
		Info,
		Loader2,
		X,
	} from "@lucide/svelte";
	import {
		Toaster as Sonner,
		type ToasterProps as SonnerProps,
	} from "svelte-sonner";

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

  Icons are Lucide only (the rest of the app is Lucide-only by design
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

    /* Pin the close button to the top-right corner *inside* the card. Sonner's
       default is a floating circle that sits half-outside the top-left edge
       (--toast-close-button-start: 0, transform: translate(-35%, -35%)) — we
       override every var that drives its position so it lands at top-right,
       inset 8px on each axis, matching the &lt;X&gt; affordance on the
       auto-updater / what's-new corner cards. */
    --toast-close-button-start: unset;
    --toast-close-button-end: 0;
    --toast-close-button-transform: translate(-8px, 8px);
  "
	toastOptions={{
		classes: {
			toast:
				"!w-[320px] !rounded-xl !border !border-border !bg-card !shadow-lg !ring-1 !ring-black/5 !p-3 !gap-3",
			content: "!gap-0.5",
			title:
				"!text-[12.5px] !font-semibold !leading-tight !text-foreground !tracking-tight",
			description: "!text-[11.5px] !text-muted-foreground !leading-snug",
			icon:
				"!size-8 !shrink-0 !flex !items-center !justify-center !rounded-lg !bg-primary/10 !text-primary !ring-1 !ring-inset !ring-primary/20 !m-0",
			closeButton:
				"!size-5 !rounded-md !border-0 !bg-transparent !text-muted-foreground/70 hover:!bg-foreground/5 hover:!text-foreground",
			actionButton: "!text-[11px] !font-semibold",
			cancelButton: "!text-[11px] !text-muted-foreground",
			success:
				"[&_[data-icon]]:!bg-success/10 [&_[data-icon]]:!text-success [&_[data-icon]]:!ring-success/25",
			error:
				"[&_[data-icon]]:!bg-destructive/10 [&_[data-icon]]:!text-destructive [&_[data-icon]]:!ring-destructive/25",
			warning:
				"[&_[data-icon]]:!bg-warning/10 [&_[data-icon]]:!text-warning [&_[data-icon]]:!ring-warning/25",
			info:
				"[&_[data-icon]]:!bg-info/10 [&_[data-icon]]:!text-info [&_[data-icon]]:!ring-info/25",
		},
	}}
	{...restProps}
>
	{#snippet loadingIcon()}
		<Loader2 class="size-4 animate-spin" />
	{/snippet}
	{#snippet successIcon()}
		<CheckCircle2 class="size-4" />
	{/snippet}
	{#snippet errorIcon()}
		<AlertOctagon class="size-4" />
	{/snippet}
	{#snippet infoIcon()}
		<Info class="size-4" />
	{/snippet}
	{#snippet warningIcon()}
		<AlertTriangle class="size-4" />
	{/snippet}
	{#snippet closeIcon()}
		<X class="size-3" />
	{/snippet}
</Sonner>
