<script lang="ts">
	import { Button } from "$components/ui/button";
	import * as Dialog from "$components/ui/dialog";
	import { AlertTriangle } from "@lucide/svelte";

	interface Props {
		open: boolean;
		title: string;
		description?: string;
		confirmLabel?: string;
		cancelLabel?: string;
		variant?: "default" | "destructive";
		/** Called on confirm. Throw or reject to keep the dialog open with the error displayed. */
		onConfirm: () => void | Promise<void>;
		onOpenChange: (open: boolean) => void;
	}

	let {
		open = $bindable(false),
		title,
		description,
		confirmLabel = "Confirm",
		cancelLabel = "Cancel",
		variant = "default",
		onConfirm,
		onOpenChange,
	}: Props = $props();

	let error = $state<string | null>(null);
	let busy = $state(false);

	$effect(() => {
		if (open) {
			error = null;
			busy = false;
		}
	});

	async function confirm() {
		if (busy) return;
		busy = true;
		error = null;
		try {
			await onConfirm();
			close();
		} catch (e) {
			error = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
			busy = false;
		}
	}

	function close() {
		open = false;
		onOpenChange(false);
	}

	function handleKeydown(e: KeyboardEvent) {
		e.stopPropagation();
		if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			confirm();
		}
		if (e.key === "Escape") {
			e.preventDefault();
			close();
		}
	}
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
		class="top-[30%] max-w-md translate-y-0 overflow-hidden rounded-xl p-0 ring-1 ring-border"
	>
		<Dialog.Header class="sr-only">
			<Dialog.Title>{title}</Dialog.Title>
			{#if description}
				<Dialog.Description>{description}</Dialog.Description>
			{/if}
		</Dialog.Header>

		<div class="flex items-start gap-3 px-4 py-4" onkeydown={handleKeydown} role="alertdialog" tabindex="-1">
			{#if variant === "destructive"}
				<div class="flex size-8 shrink-0 items-center justify-center rounded-md border border-destructive/20 bg-destructive/10 text-destructive">
					<AlertTriangle size={14} />
				</div>
			{/if}
			<div class="min-w-0 flex-1">
				<h3 class="text-[13px] font-semibold tracking-tight text-foreground">{title}</h3>
				{#if description}
					<p class="mt-1 text-[11px] leading-relaxed text-muted-foreground">{description}</p>
				{/if}
				{#if error}
					<p class="mt-2 text-[11px] text-destructive">{error}</p>
				{/if}
			</div>
		</div>

		<footer
			class="flex h-10 items-center justify-between gap-2 border-t border-border bg-muted/30 px-3 text-[11px] text-muted-foreground"
		>
			<div class="flex items-center gap-3">
				<span class="hidden items-center gap-1 sm:flex">
					<kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">⌘↵</kbd>
					<span>Confirm</span>
				</span>
				<span class="flex items-center gap-1">
					<kbd class="rounded border border-border bg-background px-1.5 py-0.5 font-mono">esc</kbd>
					<span>Cancel</span>
				</span>
			</div>
			<div class="flex items-center gap-1.5">
				<Button variant="ghost" size="xs" onclick={close} disabled={busy}>
					{cancelLabel}
				</Button>
				<Button
					variant={variant === "destructive" ? "destructive" : "default"}
					size="xs"
					onclick={confirm}
					disabled={busy}
				>
					{busy ? "Working…" : confirmLabel}
				</Button>
			</div>
		</footer>
	</Dialog.Content>
</Dialog.Root>
