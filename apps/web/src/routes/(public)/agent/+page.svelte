<script lang="ts">
import { ListChecks, ShieldCheck, Sparkles, Terminal } from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import { Container, Footer, Reveal, Section, SectionLabel, SeoMeta } from "$lib/components";
import { capabilities, connect, guarantees, model } from "./data";

const modelGlyph = ["text-tag-tangerine", "text-tag-lavender", "text-tag-green"];
const ARCH_URL = "/architecture/agentic-edits-mcp";
const REPO_URL = "https://github.com/kanakkholwal/recast";
</script>

<SeoMeta
  title="Edit recordings through your agent"
  description="Connect Claude Code, Claude Desktop or Cursor over MCP. The agent reads the take, proposes cuts, zooms and captions on a branch, and hands it back for you to apply."
  eyebrow="Agent"
/>

<main class="text-foreground">
  <section class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40">
    <Container class="pb-12">
      <Reveal variant="up">
        <div class="flex items-center gap-3">
          <SectionLabel icon={Sparkles} label="Agent" />
          <span
            class="rounded-full bg-tag-tangerine/12 px-2 py-0.5 text-caption font-medium text-tag-tangerine"
          >
            Preview
          </span>
        </div>
      </Reveal>
      <Reveal variant="up" delay={60} class="mt-5">
        <h1 class="max-w-3xl font-semibold font-display text-balance text-heading-lg md:text-display">
          Your recording is a document your agent can edit.
        </h1>
      </Reveal>
      <Reveal variant="up" delay={120} class="mt-4">
        <p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
          Connect your AI client over MCP. It reads the take, proposes cuts, zooms and captions on a
          branch, and hands it back for you to apply.
        </p>
      </Reveal>
      <Reveal variant="up" delay={180} class="mt-8 flex flex-wrap items-center gap-3">
        <Button href={ARCH_URL} variant="dark">How it works</Button>
        <Button href={REPO_URL} variant="outline" target="_blank" class="gap-2">
          <GithubBrand class="size-4" /> View on GitHub
        </Button>
      </Reveal>
    </Container>

    <Container class="border-t border-border-low">
      <div class="flex flex-wrap items-center gap-x-4 gap-y-3 py-4">
        <span class="inline-flex items-center gap-2 text-body-sm text-muted-foreground">
          <Terminal class="size-4 shrink-0" /> Add the server
        </span>
        <code
          class="rounded-lg border border-border-low bg-paper px-3 py-1.5 font-mono text-caption text-foreground"
        >
          {connect.command}
        </code>
        <span class="text-caption text-muted-foreground">
          Works with {connect.clients.join(", ")}.
        </span>
      </div>
    </Container>
  </section>

  <!-- The three beats: see, propose, apply. -->
  <section class="mx-auto w-full max-w-6xl border-b border-border-low">
    <Container>
      <div class="grid grid-cols-1 gap-px bg-border-low lg:grid-cols-3">
        {#each model as beat, i (beat.title)}
          {@const Icon = beat.icon}
          <Reveal
            variant="up"
            delay={i * 80}
            as="article"
            class="flex h-full flex-col bg-background px-6 py-10"
          >
            <Icon class={cn("size-5 [fill-opacity:0.2]", modelGlyph[i])} fill="currentColor" />
            <h2 class="mt-5 font-display text-subheading font-medium text-foreground">
              {beat.title}
            </h2>
            <p class="mt-2 text-pretty text-body-sm text-muted-foreground">{beat.description}</p>
            <ul
              class="mt-5 flex flex-wrap items-center gap-x-3 gap-y-1 text-caption text-muted-foreground"
            >
              {#each beat.tags as tag, ti (tag)}
                <li class="inline-flex items-center gap-3">
                  {#if ti > 0}
                    <span aria-hidden="true" class="text-border-strong">·</span>
                  {/if}
                  {tag}
                </li>
              {/each}
            </ul>
          </Reveal>
        {/each}
      </div>
    </Container>
  </section>

  <Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
    <Container>
      <Reveal variant="up">
        <div class="flex items-center gap-4 border-b border-border-low pb-5">
          <SectionLabel icon={ListChecks} label="What it can do" accent="lavender" />
        </div>
      </Reveal>

      <div class="max-w-lg py-10">
        <Reveal variant="up" delay={60}>
          <h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
            See, edit, review
          </h2>
        </Reveal>
        <Reveal variant="up" delay={120} class="mt-4">
          <p class="text-pretty text-body-lg text-muted-foreground">
            Every tool the agent gets, and what it is for.
          </p>
        </Reveal>
      </div>

      <div
        class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low sm:grid-cols-2 lg:grid-cols-4"
      >
        {#each capabilities as item, i (item.title)}
          {@const Icon = item.icon}
          <Reveal
            variant="up"
            delay={Math.min(i, 8) * 40}
            as="article"
            class="flex h-full flex-col bg-background px-6 py-8"
          >
            <div class="flex items-center justify-between gap-3">
              <Icon class="size-5 text-tag-tangerine [fill-opacity:0.2]" fill="currentColor" />
              <span class="text-caption text-border-strong">{item.tag}</span>
            </div>
            <h3 class="mt-4 font-display text-body font-medium text-foreground">{item.title}</h3>
            <p class="mt-2 text-body-sm text-muted-foreground">{item.description}</p>
          </Reveal>
        {/each}
      </div>
    </Container>
  </Section>

  <Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
    <Container>
      <Reveal variant="up">
        <div class="flex items-center gap-4 border-b border-border-low pb-5">
          <SectionLabel icon={ShieldCheck} label="Safe by construction" accent="green" />
        </div>
      </Reveal>

      <div class="grid items-start gap-10 py-10 md:grid-cols-12 md:gap-12">
        <div class="md:col-span-5">
          <Reveal variant="up" delay={60}>
            <h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
              The editor holds the truth
            </h2>
          </Reveal>
          <Reveal variant="up" delay={120} class="mt-4">
            <p class="text-pretty text-body-lg text-muted-foreground">
              An agent can propose anything. It can commit nothing.
            </p>
          </Reveal>
        </div>

        <ul
          class="divide-y divide-border-low border-y border-border-low md:col-span-6 md:col-start-7"
        >
          {#each guarantees as g, i (g.title)}
            <Reveal variant="up" delay={i * 70} as="li" class="flex items-start gap-3 py-4">
              <span
                aria-hidden="true"
                class="mt-1.5 size-1.5 shrink-0 rounded-full bg-tag-green"
              ></span>
              <div class="min-w-0">
                <div class="text-body font-medium text-foreground">{g.title}</div>
                <div class="mt-1 text-body-sm text-muted-foreground">{g.description}</div>
              </div>
            </Reveal>
          {/each}
        </ul>
      </div>
    </Container>
  </Section>

  <Section id="cta" class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
    <Container>
      <div class="max-w-2xl">
        <Reveal variant="up">
          <h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
            Point your agent at the take
          </h2>
        </Reveal>
        <Reveal variant="up" delay={80} class="mt-4">
          <p class="max-w-md text-pretty text-body-lg text-muted-foreground">
            Install the desktop app, add the MCP server, and ask your agent to tighten the
            recording. You approve what ships.
          </p>
        </Reveal>
        <Reveal variant="up" delay={140} class="mt-8 flex flex-wrap items-center gap-3">
          <Button href="/download" variant="dark">Download free</Button>
          <Button href={ARCH_URL} variant="outline">Read the architecture</Button>
        </Reveal>
      </div>
    </Container>
  </Section>

  <Footer />
</main>
