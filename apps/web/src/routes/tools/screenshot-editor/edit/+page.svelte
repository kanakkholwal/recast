<script lang="ts">
  import { ScreenshotEditor } from "@recast/application/screenshot-editor";
  import { toast } from "@recast/ui/sonner";
  import { Button } from "@recast/ui/button";
  import { GithubBrand } from "@recast/ui/brand-icons";
  import { ArrowLeft, Download } from "@lucide/svelte";
  import { SeoMeta } from "$lib/components";
  import { GITHUB_URL } from "$lib/components/nav-data";
  import Logo from "$lib/logo.svelte";
  import { EDITOR_DESCRIPTION } from "$lib/tools/screenshot-editor";

  // Web has no native screen capture; the editor falls back to upload/paste/drop.
  function notify(message: string, kind: "success" | "error") {
    if (kind === "success") toast.success(message);
    else toast.error(message);
  }
</script>

<!-- The landing page is the indexable one; this is the app. Point canonical at
     the landing so the two don't compete for the same query. -->
<SeoMeta
  title="Screenshot Editor"
  description={EDITOR_DESCRIPTION}
  eyebrow="Tools"
  canonicalPath="/tools/screenshot-editor"
/>

<!-- This route is chromeless (see layout.logic.ts): no marketing navbar, because
     the editor is a full-height app. It carries its own slim top bar instead. -->
<div class="flex h-dvh flex-col overflow-hidden">
  <!-- Three-column grid, not justify-between: the centre stays optically centred
       even as the side clusters change width (a Back label appearing, a longer
       CTA on wider screens), so nothing shifts under the pointer. -->
  <header
    class="border-border/60 bg-card grid h-12 shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-3 border-b px-3"
  >
    <div class="flex min-w-0 items-center gap-1">
      <Button
        variant="ghost"
        size="sm"
        href="/tools/screenshot-editor"
        aria-label="Back to the screenshot editor overview"
      >
        <ArrowLeft />
        <span class="hidden sm:inline">Back</span>
      </Button>
    </div>

    <!-- Identity in the centre: the app shell says what this IS, while the
         editor's own header below owns what you can DO. Keeping the two apart
         means neither competes with the other. -->
    <a
      href="/"
      class="group/logo focus-visible:ring-primary/40 flex min-w-0 items-center gap-2 rounded-sm outline-none focus-visible:ring-2"
      aria-label="Recast home"
    >
      <!-- Same treatment as the site Navbar: the cell is `bg-foreground` and the
           bars are `text-background`, so it inverts with the theme (black cell on
           light, white cell on dark). -->
      <span
        class="bg-foreground text-background shadow-craft-sm grid size-7 shrink-0 place-items-center rounded-lg p-1 transition-transform group-hover/logo:rotate-[-4deg]"
      >
        <Logo size="20" color="transparent" fill="currentColor" />
      </span>
      <!-- On a phone the mark alone carries the identity; the wordmark would
           squeeze the two action clusters. -->
      <span class="text-foreground hidden truncate text-sm font-semibold tracking-tight sm:inline">
        Screenshot Editor
      </span>
    </a>

    <div class="flex shrink-0 items-center justify-end gap-1">
      <!-- Secondary: GitHub steps aside on small screens so the primary CTA
           keeps a comfortable target. It is still in the landing page footer. -->
      <Button
        variant="ghost"
        size="icon"
        href={GITHUB_URL}
        target="_blank"
        rel="noopener noreferrer"
        aria-label="Recast on GitHub"
        title="Recast on GitHub"
        class="hidden sm:inline-flex"
      >
        <GithubBrand class="size-4" />
      </Button>

      <Button size="sm" href="/download">
        <Download />
        <span class="hidden lg:inline">Get the desktop app</span>
        <span class="lg:hidden">Download</span>
      </Button>
    </div>
  </header>

  <main class="min-h-0 flex-1">
    <ScreenshotEditor onnotify={notify} class="h-full" />
  </main>
</div>
