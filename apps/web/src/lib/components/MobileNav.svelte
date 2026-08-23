<script lang="ts">
import { ArrowRight, ArrowUpRight, ChevronDown, LayoutDashboard } from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import * as Collapsible from "@recast/ui/collapsible";
import * as Sheet from "@recast/ui/sheet";
import { cn } from "@recast/ui/utils";
import Logo from "$lib/logo.svelte";
import { GITHUB_URL, type MenuGroup, type NavLink } from "./nav-data";

/**
 * Mobile navigation. A full-height sheet rather than the old floating card:
 * the site has three groups and a dozen destinations now, which does not fit
 * in a popover, and a sheet gives focus trapping and a real close affordance.
 */
let {
	open = $bindable(false),
	groups,
	links,
	signedIn,
	pathname,
}: {
	open?: boolean;
	groups: MenuGroup[];
	links: NavLink[];
	signedIn: boolean;
	pathname: string;
} = $props();

// One section open at a time keeps the whole list within a thumb's reach.
let openGroup = $state(0);

const isCurrent = (href: string) => pathname === href || pathname.startsWith(`${href}/`);
const close = () => (open = false);
</script>

<Sheet.Root bind:open>
	<Sheet.Content
		side="right"
		showCloseButton
		class="w-full gap-0 border-border-low bg-background p-0 sm:max-w-sm"
	>
		<Sheet.Header class="border-b border-border-low px-5 py-4">
			<Sheet.Title class="flex items-center gap-2.5">
				<span class="grid size-7 place-items-center rounded-lg bg-foreground p-1 text-background">
					<Logo size="20" color="transparent" fill="currentColor" />
				</span>
				Recast
			</Sheet.Title>
		</Sheet.Header>

		<nav aria-label="Mobile" class="flex-1 overflow-y-auto px-3 py-3">
			{#each groups as group, i (group.label)}
				<Collapsible.Root
					bind:open={() => openGroup === i, (v) => (openGroup = v ? i : -1)}
					class="group/sec border-b border-border-low last:border-b-0"
				>
					<Collapsible.Trigger
						class="flex min-h-12 w-full cursor-pointer items-center justify-between gap-4 px-2 text-left text-body font-medium text-foreground"
					>
						{group.label}
						<ChevronDown
							class="size-4 shrink-0 text-muted-foreground transition-transform duration-300 ease-[cubic-bezier(0.625,0.05,0,1)] group-data-[state=open]/sec:rotate-180 motion-reduce:transition-none"
						/>
					</Collapsible.Trigger>
					<Collapsible.Content>
						<ul class="pb-2">
							{#each group.items as item (item.href)}
								{@const Icon = item.icon}
								<li>
									<a
										href={item.href}
										target={item.external ? "_blank" : undefined}
										rel={item.external ? "noopener noreferrer" : undefined}
										onclick={close}
										aria-current={isCurrent(item.href) ? "page" : undefined}
										class={cn(
											"flex min-h-12 items-center gap-3 rounded-lg px-2 py-2 transition-colors motion-reduce:transition-none",
											isCurrent(item.href) ? "bg-paper" : "hover:bg-paper",
										)}
									>
										<Icon class="size-4 shrink-0 text-muted-foreground" />
										<span class="min-w-0 flex-1">
											<span class="flex items-center gap-1 text-body-sm font-medium text-foreground">
												{item.label}
												{#if item.external}
													<ArrowUpRight class="size-3 text-muted-foreground" />
												{/if}
											</span>
											<span class="mt-0.5 block text-caption text-muted-foreground">
												{item.description}
											</span>
										</span>
									</a>
								</li>
							{/each}
						</ul>
					</Collapsible.Content>
				</Collapsible.Root>
			{/each}

			<ul class="mt-1 border-t border-border-low pt-1">
				{#each links as link (link.href)}
					<li>
						<a
							href={link.href}
							onclick={close}
							aria-current={isCurrent(link.href) ? "page" : undefined}
							class={cn(
								"flex min-h-12 items-center rounded-lg px-2 text-body font-medium text-foreground transition-colors motion-reduce:transition-none",
								isCurrent(link.href) ? "bg-paper" : "hover:bg-paper",
							)}
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
						class="flex min-h-12 items-center gap-2 rounded-lg px-2 text-body font-medium text-foreground transition-colors hover:bg-paper motion-reduce:transition-none"
					>
						<GithubBrand class="size-4 text-muted-foreground" />
						GitHub
						<ArrowUpRight class="size-3 text-muted-foreground" />
					</a>
				</li>
			</ul>
		</nav>

		<!-- Pinned: the two actions worth a thumb, always reachable without -->
		<!-- scrolling back up. -->
		<Sheet.Footer class="gap-2 border-t border-border-low px-5 py-4">
			{#if signedIn}
				<Button href="/dashboard" variant="dark" class="w-full gap-2" onclick={close}>
					<LayoutDashboard class="size-4" />
					Go to dashboard
				</Button>
			{:else}
				<Button href="/download" variant="dark" class="group/cta w-full gap-2" onclick={close}>
					Download Recast
					<ArrowRight
						class="size-4 transition-transform group-hover/cta:translate-x-0.5 motion-reduce:transition-none"
					/>
				</Button>
				<Button href="/login" variant="outline" class="w-full" onclick={close}>Sign in</Button>
			{/if}
		</Sheet.Footer>
	</Sheet.Content>
</Sheet.Root>
