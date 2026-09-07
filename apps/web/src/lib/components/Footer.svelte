<script lang="ts">
import { Container } from "$lib/components";
import { footerCols as cols, footerSocials as socials } from "$lib/components/nav-data";
import Logo from "$lib/logo.svelte";
</script>

<footer class="relative border-t border-border-low bg-paper">
  <Container class="relative pt-20 pb-10 md:pt-24 md:pb-12" as="div">
    <div class="grid gap-14 md:grid-cols-12">
      <div class="md:col-span-5">
        <a href="/" class="inline-flex items-center gap-2.5">
          <span
            class="grid size-8 place-items-center rounded-lg bg-foreground p-1 text-background"
          >
            <Logo size="22" color="transparent" fill="currentColor" />
          </span>
          <span class="text-subheading font-semibold text-foreground">
            Recast
          </span>
        </a>
        <p class="mt-6 max-w-sm text-pretty text-body-sm text-muted-foreground">
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
              class="grid size-9 place-items-center rounded-lg border border-border-low bg-card text-muted-foreground transition-colors hover:border-border-strong hover:text-foreground"
            >
              <Icon class="size-4" />
            </a>
          {/each}
        </div>
        <p class="mt-7 text-caption text-muted-foreground">
          © {new Date().getFullYear()} Recast. All rights reserved.
        </p>
      </div>

      <div class="grid gap-10 sm:grid-cols-3 md:col-span-7">
        {#each cols as col}
          <div>
            <h4 class="text-body-sm font-semibold text-foreground">
              {col.title}
            </h4>
            <ul class="mt-4 space-y-3">
              {#each col.links as link}
                <li>
                  <a
                    href={link.href}
                    target={link.external ? "_blank" : undefined}
                    rel={link.external ? "noopener noreferrer" : undefined}
                    class="text-body-sm text-muted-foreground transition-colors hover:text-foreground"
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
  </Container>

  <div class="relative overflow-hidden px-4 pb-8 md:pb-10">
    <span
      class="wordmark block select-none text-center font-display text-[22vw] font-medium leading-[0.82] tracking-tight"
    >
      Recast
    </span>
  </div>
</footer>

<style>
	/* Foreground-to-neutral fill with a highlight band that drifts across it. */
	.wordmark {
		background-image: linear-gradient(
			100deg,
			color-mix(in oklab, var(--color-foreground) 4%, transparent) 0%,
			color-mix(in oklab, var(--color-foreground) 26%, transparent) 20%,
			color-mix(in oklab, var(--color-foreground) 80%, transparent) 50%,
			color-mix(in oklab, var(--color-foreground) 26%, transparent) 80%,
			color-mix(in oklab, var(--color-foreground) 4%, transparent) 100%
		);
		background-size: 260% 100%;
		background-clip: text;
		-webkit-background-clip: text;
		color: transparent;
		animation: wordmark-sheen 9s linear infinite;
	}

	@keyframes wordmark-sheen {
		from {
			background-position: 160% 0;
		}
		to {
			background-position: -60% 0;
		}
	}

	/* The global guard collapses duration to 0.01ms, parking the sheen mid-sweep, so kill it and hold the flat fill. */
	@media (prefers-reduced-motion: reduce) {
		.wordmark {
			animation: none;
			background-image: linear-gradient(
				100deg,
				color-mix(in oklab, var(--color-foreground) 5%, transparent) 0%,
				color-mix(in oklab, var(--color-foreground) 12%, transparent) 100%
			);
		}
	}
</style>
