<script lang="ts">
  import { GITHUB_URL, navMenuGroups as menuGroups } from "$lib/components/nav-data";
  import Logo from "$lib/logo.svelte";
  import { FloatingMenu } from "$lib/motion-core";
  import { GithubBrand } from "@recast/ui/brand-icons";

  let menuContainer: HTMLElement;
</script>

<div
  bind:this={menuContainer}
  class="fixed inset-x-0 top-4 z-50 flex justify-center px-4"
>
  <FloatingMenu
    portalTarget={menuContainer}
    {menuGroups}
    primaryButton={{ label: "Download", href: "/download" }}
    secondaryButton={{ label: "Cloud Waitlist", href: "/waitlist" }}
    tertiaryButton={{
      label: "GitHub",
      href: GITHUB_URL,
      icon: GithubBrand,
      iconOnly: true,
      ariaLabel: "Recast on GitHub",
    }}
    classes={{
      root: "border-border/40 bg-card/60 shadow-craft-floating ring-1 ring-border-low/60 backdrop-blur-xl backdrop-saturate-150 supports-[backdrop-filter]:bg-card/40",
      header: "px-4 py-2",
    }}
  >
    {#snippet logo()}
      <a
        href="/"
        class="group/logo flex items-center gap-2.5 transition-transform active:scale-[0.97]"
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
    {/snippet}
  </FloatingMenu>
</div>
