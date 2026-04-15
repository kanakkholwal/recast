<script lang="ts">
  import { page } from "$app/stores";
  import CustomTitlebar from "$components/layout/custom-titlebar.svelte";
  import { config } from "$constants/app";
  import {
    AlertTriangle,
    ArrowLeft,
    Construction,
    FileQuestion,
    Home,
    RefreshCcw,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";

  $: status = $page.status;
  $: message = $page.error?.message || "An unexpected error occurred.";

  $: isNotFound = status === 404;
  $: isServerError = status >= 500;

  $: errorTitle = isNotFound
    ? "Page not found"
    : isServerError
      ? "Server Error"
      : "Something went wrong";

  $: errorDesc = isNotFound
    ? "Sorry, we couldn't find the page you're looking for."
    : "Our servers ran into a bit of a hiccup. We're working on fixing it.";

  function goBack() {
    history.back();
  }
</script>

<CustomTitlebar wrapperClass="h-7">
  <div class="flex items-center gap-2 px-2 h-full" data-tauri-drag-region>
    <span class="text-[11px] font-semibold text-foreground">{config.appName}</span>
  </div>
</CustomTitlebar>

<div class="flex min-h-[calc(100vh-28px)] w-full items-center justify-center px-6">
  <div class="flex w-full max-w-sm flex-col items-center gap-4 text-center">
    <div
      class="flex size-12 items-center justify-center rounded-lg border border-border bg-muted/40"
    >
      {#if isNotFound}
        <FileQuestion class="size-5 text-info" strokeWidth={1.75} />
      {:else if isServerError}
        <Construction class="size-5 text-warning" strokeWidth={1.75} />
      {:else}
        <AlertTriangle class="size-5 text-destructive" strokeWidth={1.75} />
      {/if}
    </div>

    <p class="font-mono text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
      Error {status}
    </p>

    <h1 class="text-[15px] font-semibold tracking-tight text-foreground">
      {errorTitle}
    </h1>

    <p class="text-[12px] text-muted-foreground">
      {errorDesc}
    </p>

    <pre
      class="w-full max-w-xs overflow-hidden truncate rounded border border-border bg-muted/40 px-2 py-1 text-left font-mono text-[10px] text-muted-foreground"
      title={message}>{message}</pre>

    <div class="flex w-full items-center gap-1.5">
      <Button variant="outline" size="sm" onclick={goBack} class="flex-1 gap-1.5">
        <ArrowLeft size={13} />
        Go Back
      </Button>
      <Button href="/" variant="default" size="sm" class="flex-1 gap-1.5">
        <Home size={13} />
        Home
      </Button>
    </div>

    {#if !isNotFound}
      <Button
        variant="ghost"
        size="xs"
        onclick={() => location.reload()}
        class="gap-1.5 text-muted-foreground"
      >
        <RefreshCcw size={11} />
        Try reloading
      </Button>
    {/if}

    <a
      href="mailto:{config.supportEmail}"
      class="text-[11px] text-muted-foreground hover:text-foreground hover:underline"
    >
      Need help? Contact support
    </a>
  </div>
</div>
