<script lang="ts">
import { Archive, BarChart3, Film, LayoutDashboard, Plus, Users } from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import * as Sidebar from "@recast/ui/sidebar";
import { useSidebar } from "@recast/ui/sidebar";
import { cn } from "@recast/ui/utils";
import type { ComponentProps } from "svelte";
import { cubicOut } from "svelte/easing";
import { crossfade, fade } from "svelte/transition";
import { page } from "$app/state";
import { GITHUB_URL } from "$lib/components/nav-data";
import OrgSwitcher from "$lib/dashboard/components/OrgSwitcher.svelte";
import { quickUpload } from "$lib/dashboard/quick-upload.svelte";
import Logo from "$lib/logo.svelte";
import { isActive, resolveActiveOrg, resolveMemberships } from "./DashboardSidebar.logic";

// One shape powers the dashboard and admin shells; every @recast/icons icon shares a component type, so no `any` is needed.
interface NavItem {
	title: string;
	href: string;
	icon: typeof LayoutDashboard;
	exact: boolean;
}

interface NavGroup {
	label: string;
	items: NavItem[];
}

interface Props {
	/** Grouped nav (dashboard). Takes precedence over `nav`. */
	groups?: NavGroup[];
	/** Flat nav rows (admin shell). Rendered as a single group. */
	nav?: NavItem[];
	/** Small label under the wordmark ("Dashboard" / "Admin"). */
	subtitle?: string;
	/** Section heading for the flat `nav` group. */
	groupLabel?: string;
	/** Where the wordmark links. */
	homeHref?: string;
	/** Show the org switcher + "New Recast" CTA (dashboard only). */
	showOrgSwitcher?: boolean;
}

// Primary workspace destinations up top, media library below; Settings lives in the profile menu, not the rail.
const defaultGroups: NavGroup[] = [
	{
		label: "Workspace",
		items: [
			{
				title: "Dashboard",
				href: "/dashboard",
				icon: LayoutDashboard,
				exact: true,
			},
			{
				title: "Analytics",
				href: "/dashboard/analytics",
				icon: BarChart3,
				exact: false,
			},
			{ title: "Team", href: "/dashboard/team", icon: Users, exact: false },
		],
	},
	{
		label: "Library",
		items: [
			{
				title: "Recasts",
				href: "/dashboard/recasts",
				icon: Film,
				exact: false,
			},
			{
				title: "Archive",
				href: "/dashboard/archive",
				icon: Archive,
				exact: false,
			},
		],
	},
];

let {
	groups,
	nav,
	subtitle = "Dashboard",
	groupLabel = "Menu",
	homeHref = "/dashboard",
	showOrgSwitcher = true,
}: Props = $props();

const sidebar = useSidebar();
const open = $derived(sidebar.state === "expanded");
const currentPath = $derived(page.url.pathname);

// Grouped nav wins, a flat `nav` becomes one group, else the dashboard default; the admin shell stays unchanged.
const resolvedGroups = $derived<NavGroup[]>(
	groups ?? (nav ? [{ label: groupLabel, items: nav }] : defaultGroups),
);

const memberships = $derived(resolveMemberships(page.data));
const activeOrg = $derived(resolveActiveOrg(page.data));

// Slides the active highlight between rows rather than cross-fading in place.
const [send, receive] = crossfade({
	duration: 280,
	easing: cubicOut,
	fallback: (node) => fade(node, { duration: 120 }),
});
</script>

<Sidebar.Root variant="inset" collapsible="icon">
  <Sidebar.Rail class="data-[state=collapsed]:hidden" />

  <Sidebar.Header class="gap-3 py-3">
    <a
      href={homeHref}
      aria-label="Recast {subtitle}"
      class={cn(
        "flex h-10 items-center overflow-hidden rounded-lg transition-[padding,opacity] duration-200 ease-linear hover:opacity-80",
        open ? "px-1.5" : "px-0",
      )}
    >
      <span
        class="grid size-8 shrink-0 place-items-center rounded-lg bg-foreground p-1 text-background shadow-craft-sm"
      >
        <Logo size="20" color="transparent" fill="currentColor" />
      </span>

      <span
        class={cn(
          "flex h-8 flex-col justify-center overflow-hidden leading-none transition-[max-width,margin,opacity] duration-200 ease-linear",
          open ? "ml-2.5 max-w-32 opacity-100" : "ml-0 max-w-0 opacity-0",
        )}
      >
        <span
          class="truncate font-display text-sm font-semibold text-foreground"
        >
          Recast
        </span>
        <span
          class="mt-0.5 truncate text-caption text-muted-foreground"
        >
          {subtitle}
        </span>
      </span>
    </a>

    {#if showOrgSwitcher && activeOrg}
      <div class="mt-1 pt-2">
        <OrgSwitcher
          memberships={memberships.map((m) => ({
            organizationId: m.organizationId,
            name: m.name,
            role: m.role,
            plan: m.plan,
          }))}
          active={activeOrg}
        />
      </div>
    {/if}

    {#if showOrgSwitcher}
      <Button
        type="button"
        size="sm"
        variant="dark"
        onclick={() => quickUpload.show()}
        class={cn(
          "group/new h-9 w-full gap-2.5 overflow-hidden rounded-lg px-2.5",
          open ? "justify-center" : "justify-start",
        )}
        title="New Recast"
      >
        <Plus
          size={14}
          class="shrink-0 transition-transform duration-200 group-hover/new:rotate-90"
        />
        <span
          class={cn(
            "overflow-hidden text-body-sm font-medium transition-[max-width,opacity] duration-200 ease-linear",
            open ? "max-w-32 opacity-100" : "max-w-0 opacity-0",
          )}
        >
          New Recast
        </span>
      </Button>
    {/if}
  </Sidebar.Header>

  <Sidebar.Content class="scrollbar-hide">
    {#each resolvedGroups as group (group.label)}
      <Sidebar.Group>
    
        <Sidebar.GroupLabel
          class="px-2 text-caption font-medium text-muted-foreground"
        >
          {group.label}
        </Sidebar.GroupLabel>
        <Sidebar.GroupContent>
          <Sidebar.Menu class="gap-0.5">
            {#each group.items as link (link.href)}
              {@const active = isActive(link.href, link.exact, currentPath)}
              {@const Icon = link.icon}
              <Sidebar.MenuItem>
                <Sidebar.MenuButton tooltipContent={link.title}>
                  {#snippet child({
                    props,
                  }: {
                    props: ComponentProps<typeof Sidebar.MenuButton>;
                  })}
                    <a
                      href={link.href}
                      {...props as Record<string, unknown>}
                      data-active={active}
                      class={cn(
                        "group/item relative flex h-9 w-full items-center gap-2.5 overflow-hidden rounded-lg px-2.5 text-body-sm font-medium transition-colors duration-200",
                        active
                          ? "text-foreground"
                          : "text-muted-foreground hover:text-foreground",
                      )}
                    >
                      {#if active}
                        <span
                          in:receive={{ key: "nav-active-bg" }}
                          out:send={{ key: "nav-active-bg" }}
                          class="absolute inset-0 z-0 rounded-lg bg-paper ring-1 ring-inset ring-border-low"
                          aria-hidden="true"
                        ></span>
                        {#if open}
                          <span
                            in:receive={{ key: "nav-active-pill" }}
                            out:send={{ key: "nav-active-pill" }}
                            class="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full bg-primary"
                            aria-hidden="true"
                          ></span>
                        {/if}
                      {/if}
                      <Icon
                        size={14}
                        class="relative z-10 shrink-0 transition-transform duration-200 group-hover/item:-translate-y-px group-active/item:scale-95"
                      />
                      <span
                        class={cn(
                          "relative z-10 truncate transition-[max-width,opacity] duration-200 ease-linear",
                          open ? "max-w-40 opacity-100" : "max-w-0 opacity-0",
                        )}
                      >
                        {link.title}
                      </span>
                    </a>
                  {/snippet}
                </Sidebar.MenuButton>
              </Sidebar.MenuItem>
            {/each}
          </Sidebar.Menu>
        </Sidebar.GroupContent>
      </Sidebar.Group>
    {/each}
  </Sidebar.Content>

  <Sidebar.Footer class="gap-1 p-2">
    <Button
      href={GITHUB_URL}
      target="_blank"
      rel="noopener noreferrer"
      aria-label="Star Recast on GitHub"
      title="Star on GitHub"
      variant="outline"
      class={cn(
        "group/gh relative h-9 w-full gap-2.5 overflow-hidden rounded-lg px-2.5",
        open ? "justify-center" : "justify-start",
      )}
    >
        <GithubBrand
          class="size-3.5 transition-transform duration-300 group-hover/gh:scale-110"
        />
      <span
        class={cn(
          "relative z-10 inline-flex items-center gap-1.5 truncate transition-[max-width,opacity] duration-200 ease-linear",
          open ? "max-w-40 opacity-100" : "max-w-0 opacity-0",
        )}
      >
        Star on GitHub
      </span>
    </Button>
  </Sidebar.Footer>
</Sidebar.Root>

<style>
  .scrollbar-hide {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }
  .scrollbar-hide::-webkit-scrollbar {
    display: none;
  }
</style>
