<script lang="ts">
	import { mode } from "mode-watcher";
	import {
		AlertOctagon,
		AlertTriangle,
		CheckCircle2,
		Info,
		Loader2,
	} from "@lucide/svelte";
	import {
		Toaster as Sonner,
		type ToasterProps as SonnerProps,
	} from "svelte-sonner";

	let { ...restProps }: SonnerProps = $props();
</script>

<!--
  Recast desktop/web Sonner theming.

  The bare svelte-sonner ships saturated reds/greens/yellows ("rich colors")
  that clash with our muted glass UI. We pin every per-variant background,
  border and text variable to design tokens so toasts read as part of the same
  surface family as Popover and Dialog. Per-variant colors come from
  --success / --destructive / --warning / --info; never re-style toasts via
  className from a caller — call the matching `toast.success` / `.error` /
  `.warning` / `.info` instead.

  Icons are Lucide only (the rest of the app is Lucide-only by design rule),
  sized at 16px so they line up with the 14px label baseline.
-->
<Sonner
	theme={mode.current}
	class="toaster group"
	style="
    --normal-bg: var(--color-popover);
    --normal-text: var(--color-popover-foreground);
    --normal-border: color-mix(in srgb, var(--color-border) 80%, transparent);

    --success-bg: color-mix(in srgb, var(--color-success) 10%, var(--color-popover));
    --success-text: var(--color-foreground);
    --success-border: color-mix(in srgb, var(--color-success) 35%, transparent);

    --error-bg: color-mix(in srgb, var(--color-destructive) 10%, var(--color-popover));
    --error-text: var(--color-foreground);
    --error-border: color-mix(in srgb, var(--color-destructive) 35%, transparent);

    --warning-bg: color-mix(in srgb, var(--color-warning) 12%, var(--color-popover));
    --warning-text: var(--color-foreground);
    --warning-border: color-mix(in srgb, var(--color-warning) 35%, transparent);

    --info-bg: color-mix(in srgb, var(--color-info) 10%, var(--color-popover));
    --info-text: var(--color-foreground);
    --info-border: color-mix(in srgb, var(--color-info) 30%, transparent);
  "
	toastOptions={{
		classes: {
			toast:
				"!rounded-xl !backdrop-blur-md !shadow-[0_8px_24px_-8px_rgba(0,0,0,0.18),0_2px_4px_-2px_rgba(0,0,0,0.08)] !ring-1 !ring-inset !ring-border/40",
			title: "!text-[12.5px] !font-semibold !tracking-tight",
			description: "!text-[11px] !text-muted-foreground !leading-relaxed",
			actionButton: "!text-[11px] !font-semibold",
			cancelButton: "!text-[11px] !text-muted-foreground",
			success: "[&_[data-icon]]:!text-success",
			error: "[&_[data-icon]]:!text-destructive",
			warning: "[&_[data-icon]]:!text-warning",
			info: "[&_[data-icon]]:!text-info",
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
</Sonner>
