<script lang="ts">
  import { Container } from "$lib/components";
  import { footerCols as cols, footerSocials as socials } from "$lib/components/nav-data";
  import Logo from "$lib/logo.svelte";
</script>

<footer class="relative overflow-hidden border-t border-border-low/70">
  <div class="bg-aurora absolute inset-x-0 top-0 z-10 h-px"></div>

  <!-- Editorial backdrop, mirroring the hero: the photo sits in the lower half
       and fades into the page background so the columns stay readable up top.
       Same asset as the hero; a missing file just leaves the clean gradient. -->
  <div aria-hidden="true" class="pointer-events-none absolute inset-0">
    <div
      class="absolute inset-x-0 bottom-0 h-2/3 bg-cover bg-center opacity-40 dark:opacity-25"
      style="background-image: url('/hero-background.png');"
    ></div>
    <div
      class="absolute inset-0"
      style="background: linear-gradient(to bottom, var(--color-background) 28%, transparent 78%);"
    ></div>
  </div>

  <!-- Oversized wordmark, cut off at the bottom edge: the closing beat. -->
  <div
    aria-hidden="true"
    class="pointer-events-none absolute inset-x-0 bottom-0 flex justify-center overflow-hidden"
  >
    <span
      class="translate-y-[26%] text-[24vw] font-bold leading-none tracking-tighter text-foreground/6 select-none"
    >
      Recast
    </span>
  </div>

  <Container class="relative z-10 py-20 md:py-28" as="div">
    <div class="grid gap-14 md:grid-cols-12">
      <div class="md:col-span-5">
        <a href="/" class="inline-flex items-center gap-2.5">
          <span
            class="grid size-8 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm"
          >
            <Logo size="22" color="transparent" fill="currentColor" />
          </span>
          <span class="text-lg font-semibold tracking-tight text-foreground">
            Recast
          </span>
        </a>
        <p
          class="mt-6 max-w-sm text-pretty text-sm leading-relaxed text-muted-foreground"
        >
          Turns a raw screen capture into a polished, shareable demo while you
          record. The timeline is there when you want it. Most of the time, you
          won't need it.
        </p>
        <div class="mt-7 flex items-center gap-2">
          {#each socials as { icon: Icon, href, label }}
            <a
              {href}
              aria-label={label}
              target={href.startsWith("http") ? "_blank" : undefined}
              rel={href.startsWith("http") ? "noopener noreferrer" : undefined}
              class="glass-chip grid size-9 place-items-center rounded-lg text-muted-foreground transition-all hover:-translate-y-0.5 hover:text-foreground"
            >
              <Icon class="size-4" />
            </a>
          {/each}
        </div>
      </div>

      <div class="grid gap-10 sm:grid-cols-3 md:col-span-7">
        {#each cols as col}
          <div>
            <h4
              class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground"
            >
              {col.title}
            </h4>
            <ul class="mt-5 space-y-3.5">
              {#each col.links as link}
                <li>
                  <a
                    href={link.href}
                    target={link.external ? "_blank" : undefined}
                    rel={link.external ? "noopener noreferrer" : undefined}
                    class="text-sm font-medium text-foreground/75 transition-colors hover:text-foreground"
                  >
                    {link.label}
                  </a>
                </li>
              {/each}
            </ul>
          </div>
        {/each}
      </div>
    </div>

    <div
      class="mt-20 flex flex-col items-start justify-between gap-4 border-t border-border-low/70 pt-8 md:flex-row md:items-center"
    >
      <p
        class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground"
      >
        Record once · Ship the demo
      </p>
      <div
        class="flex flex-col items-start gap-3 sm:flex-row sm:items-center sm:gap-5"
      >
        <nav class="flex items-center gap-5">
          <a
            href="/privacy-policy"
            class="text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
          >
            Privacy Policy
          </a>
          <a
            href="/terms-of-service"
            class="text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
          >
            Terms of Service
          </a>
        </nav>
        <span class="hidden text-muted-foreground/40 sm:inline">·</span>
        <p class="text-xs text-muted-foreground">
          © {new Date().getFullYear()} Recast. All rights reserved.
        </p>
      </div>
    </div>
  </Container>
</footer>
