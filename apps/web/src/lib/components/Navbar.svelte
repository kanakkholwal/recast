<script lang="ts">
  import { GITHUB_URL, navLinks } from "$lib/components/nav-data";
  import Logo from "$lib/logo.svelte";
  import { prefersReducedMotion } from "$lib/motion-core";
  import { Menu, X } from "@lucide/svelte";
  import { GithubBrand } from "@recast/ui/brand-icons";
  import { Button } from "@recast/ui/button";
  import { slide } from "svelte/transition";

  // Minimal inline nav: brand left, links centered, Download primary right, with
  // a compact disclosure on mobile. Replaces the old hamburger-collapse menu so
  // the links are always visible, matching the refined hero direction.
  let open = $state(false);
  const reduced = $derived(prefersReducedMotion());
  const close = () => (open = false);

  const linkClass =
    "inline-flex items-center rounded-lg px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground";
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") close();
  }}
/>

<div class="fixed inset-x-0 top-4 z-50 flex justify-center px-4">
  <nav
    aria-label="Primary"
    class="glass-strong bg-card flex w-full max-w-3xl items-center gap-2 rounded-2xl p-1.5"
  >
    <a
      href="/"
      class="group/logo flex items-center gap-2.5 rounded-xl px-2 py-1 transition-transform active:scale-[0.97]"
      aria-label="Recast home"
    >
      <span
        class="grid size-7 place-items-center rounded-lg bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
      >
        <Logo size="20" color="transparent" fill="currentColor" />
      </span>
      <span class="text-[15px] font-semibold tracking-tight text-foreground">
        Recast
      </span>
    </a>

    <!-- Inline links, centered (desktop only). -->
    <ul class="hidden flex-1 items-center justify-center gap-0.5 md:flex">
      {#each navLinks as link (link.href)}
        <li>
          <a href={link.href} class={linkClass}>{link.label}</a>
        </li>
      {/each}
    </ul>

    <div class="ml-auto flex items-center gap-1.5 md:ml-0">
      <a
        href={GITHUB_URL}
        target="_blank"
        rel="noopener noreferrer"
        aria-label="Recast on GitHub"
        class="hidden size-9 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground md:grid"
      >
        <GithubBrand class="size-4" />
      </a>
      <Button href="/download" size="sm" class="gap-1.5">
        Download
      </Button>
      <button
        type="button"
        onclick={() => (open = !open)}
        aria-expanded={open}
        aria-controls="mobile-nav"
        aria-label={open ? "Close menu" : "Open menu"}
        class="grid size-9 place-items-center rounded-lg text-foreground transition-colors hover:bg-foreground/5 md:hidden"
      >
        {#if open}
          <X class="size-5" />
        {:else}
          <Menu class="size-5" />
        {/if}
      </button>
    </div>
  </nav>
</div>

{#if open}
  <!-- Click-away backdrop. -->
  <button
    type="button"
    class="fixed inset-0 z-40 md:hidden"
    aria-label="Close menu"
    tabindex="-1"
    onclick={close}
  ></button>
  <div
    id="mobile-nav"
    class="glass-strong fixed inset-x-4 top-19 z-50 rounded-2xl p-2 md:hidden"
    transition:slide={{ duration: reduced ? 0 : 200 }}
  >
    <ul class="flex flex-col">
      {#each navLinks as link (link.href)}
        <li>
          <a
            href={link.href}
            onclick={close}
            class="block rounded-lg px-3 py-2.5 text-sm font-medium text-foreground/80 transition-colors hover:bg-foreground/5 hover:text-foreground"
          >
            {link.label}
          </a>
        </li>
      {/each}
      <li>
        <a
          href={GITHUB_URL}
          target="_blank"
          rel="noopener noreferrer"
          onclick={close}
          class="block rounded-lg px-3 py-2.5 text-sm font-medium text-foreground/80 transition-colors hover:bg-foreground/5 hover:text-foreground"
        >
          GitHub
        </a>
      </li>
    </ul>
  </div>
{/if}
