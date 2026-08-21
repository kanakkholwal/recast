<script lang="ts">
import { authClient } from "$lib/auth/client";
import { GITHUB_URL, navLinks } from "$lib/components/nav-data";
import Logo from "$lib/logo.svelte";
import { prefersReducedMotion } from "$lib/motion-core";
import { LayoutDashboard, Menu, X } from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import { slide } from "svelte/transition";

let open = $state(false);
const reduced = $derived(prefersReducedMotion());
const close = () => (open = false);

let scrolled = $state(false);

const session = authClient.useSession();
const signedIn = $derived(Boolean($session.data?.user));

const linkClass =
	"inline-flex items-center whitespace-nowrap rounded-full px-4 py-2 text-body-sm font-medium text-muted-foreground transition-colors hover:text-foreground";
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") close();
  }}
  onscroll={() => (scrolled = window.scrollY > 8)}
/>

<div
  class="fixed inset-x-0 top-0 z-50 border-b transition-colors duration-200 {scrolled || open
    ? 'border-border-low bg-background'
    : 'border-transparent bg-transparent'}"
>
  <nav
    aria-label="Primary"
    class="mx-auto flex h-16 w-full max-w-6xl items-center gap-2 px-6 sm:px-8 lg:px-10"
  >
    <a
      href="/"
      class="group/logo flex shrink-0 items-center gap-2.5 rounded-lg py-1 pr-2"
      aria-label="Recast home"
    >
      <span
        class="grid size-7 place-items-center rounded-lg bg-foreground p-1 text-background"
      >
        <Logo size="20" color="transparent" fill="currentColor" />
      </span>
      <span class="whitespace-nowrap text-lg font-display tracking-wide font-semibold text-foreground">
        Recast
      </span>
    </a>

    <!-- Inline links, centered (desktop only). -->
    <ul class="hidden flex-1 items-center justify-center gap-1 md:flex">
      {#each navLinks as link (link.href)}
        <li>
          <a href={link.href} class={linkClass}>{link.label}</a>
        </li>
      {/each}
    </ul>

    <div class="ml-auto flex shrink-0 items-center gap-2 md:ml-0">
      <a
        href={GITHUB_URL}
        target="_blank"
        rel="noopener noreferrer"
        aria-label="Recast on GitHub"
        class="hidden size-9 place-items-center rounded-lg text-muted-foreground transition-colors hover:text-foreground md:grid"
      >
        <GithubBrand class="size-4" />
      </a>
      {#if signedIn}
        <a href="/dashboard" class="hidden {linkClass} md:inline-flex">
          <LayoutDashboard class="mr-1.5 size-3.5" />
          Dashboard
        </a>
      {:else}
        <a href="/login" class="hidden {linkClass} md:inline-flex">Sign in</a>
      {/if}
      <Button href="/download" size="sm" variant="dark">Download</Button>
      <button
        type="button"
        onclick={() => (open = !open)}
        aria-expanded={open}
        aria-controls="mobile-nav"
        aria-label={open ? "Close menu" : "Open menu"}
        class="grid size-9 place-items-center rounded-lg text-foreground transition-colors hover:bg-paper md:hidden"
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
    class="fixed inset-x-4 top-18 z-50 rounded-xl border border-border-low bg-card p-2 md:hidden"
    transition:slide={{ duration: reduced ? 0 : 200 }}
  >
    <ul class="flex flex-col">
      {#each navLinks as link (link.href)}
        <li>
          <a
            href={link.href}
            onclick={close}
            class="block rounded-lg px-3 py-2.5 text-body-sm font-medium text-foreground transition-colors hover:bg-paper"
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
          class="block rounded-lg px-3 py-2.5 text-body-sm font-medium text-foreground transition-colors hover:bg-paper"
        >
          GitHub
        </a>
      </li>
    </ul>

    <div class="mt-2 border-t border-border-low pt-2">
      {#if signedIn}
        <a
          href="/dashboard"
          onclick={close}
          class="block rounded-lg px-3 py-2.5 text-body-sm font-semibold text-foreground transition-colors hover:bg-paper"
        >
          Go to dashboard
        </a>
      {:else}
        <a
          href="/login"
          onclick={close}
          class="block rounded-lg px-3 py-2.5 text-body-sm font-medium text-foreground transition-colors hover:bg-paper"
        >
          Sign in
        </a>
        <a
          href="/signup"
          onclick={close}
          class="block rounded-lg px-3 py-2.5 text-body-sm font-semibold text-foreground transition-colors hover:bg-paper"
        >
          Start free
        </a>
      {/if}
    </div>
  </div>
{/if}
