<script lang="ts">
  import { page } from "$app/state";
  import OrgSwitcher from "$lib/dashboard/components/OrgSwitcher.svelte";
  import Logo from "$lib/logo.svelte";
  import {
    BarChart3,
    Film,
    LayoutDashboard,
    Moon,
    Plus,
    Sun,
    Users,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Sidebar from "@recast/ui/sidebar";
  import { useSidebar } from "@recast/ui/sidebar";
  import { mode, toggleMode } from "@recast/ui/theme";
  import { cn } from "@recast/ui/utils";
  import type { ComponentProps } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { crossfade, fade } from "svelte/transition";
  import {
    isActive,
    resolveActiveOrg,
    resolveMemberships,
  } from "./DashboardSidebar.logic";

  // A nav entry; the same shape powers both the dashboard and admin shells.
  // All Lucide icons share one component type, so `typeof LayoutDashboard`
  // accepts any of them without `any`.
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

  // Default dashboard nav, grouped like the Loom-style shell: primary
  // workspace destinations up top, the media library below. Settings lives in
  // the header profile menu + command palette, not the rail.
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

  // Grouped nav wins; a flat `nav` becomes one group; otherwise the dashboard
  // default. Keeps the admin shell (flat `nav`) working unchanged.
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
      <!-- Always mounted: the label collapses its own width (and left margin)
			     in sync with the sidebar width, so nothing snaps or clips on toggle. -->
      <span
        class={cn(
          "flex flex-col overflow-hidden leading-none transition-[max-width,margin,opacity] duration-200 ease-linear",
          open ? "ml-2.5 max-w-32 opacity-100" : "ml-0 max-w-0 opacity-0",
        )}
      >
        <span
          class="truncate text-[15px] font-semibold tracking-tight text-foreground"
        >
          Recast
        </span>
        <span
          class="mt-0.5 truncate text-[10px] font-medium uppercase tracking-[0.16em] text-muted-foreground"
        >
          {subtitle}
        </span>
      </span>
    </a>

    {#if showOrgSwitcher && activeOrg}
      <div class="mt-1 border-t border-border/30 pt-2">
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
        href="/dashboard/recasts"
        size="sm"
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
            "overflow-hidden text-[12px] font-semibold transition-[max-width,opacity] duration-200 ease-linear",
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
        <!-- Kept mounted: GroupLabel has a built-in collapse
				     (`group-data-[collapsible=icon]:-mt-8 opacity-0`, transitioned), so
				     it slides away smoothly instead of popping out of the DOM. -->
        <Sidebar.GroupLabel
          class="px-2 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
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
                        "group/item relative flex h-9 w-full items-center gap-2.5 overflow-hidden rounded-lg px-2.5 text-[12.5px] font-medium transition-colors duration-200",
                        active
                          ? "text-foreground"
                          : "text-muted-foreground hover:text-foreground",
                      )}
                    >
                      {#if active}
                        <span
                          in:receive={{ key: "nav-active-bg" }}
                          out:send={{ key: "nav-active-bg" }}
                          class="absolute inset-0 z-0 rounded-lg bg-foreground/6 ring-1 ring-inset ring-border/40"
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

  <Sidebar.Footer class="gap-1 border-t border-border/30 p-2">
    <Button
      type="button"
      variant="secondary"
      onclick={toggleMode}
      aria-label={mode.current === "dark"
        ? "Switch to light mode"
        : "Switch to dark mode"}
      title={mode.current === "dark" ? "Light mode" : "Dark mode"}
      class={cn(
        "group/theme h-9 w-full gap-2.5 overflow-hidden rounded-lg px-2.5",
        open ? "justify-center" : "justify-start",
      )}
    >
        {#if mode.current === "dark"}
            <Sun class="size-3.5 transition-transform duration-300 group-hover/theme:rotate-45" />
        {:else}     
            <Moon class="size-3.5  transition-transform duration-300 group-hover/theme:-rotate-12" />
        {/if}
      <span
        class={cn(
          "truncate transition-[max-width,opacity] duration-200 ease-linear",
          open ? "max-w-40 opacity-100" : "max-w-0 opacity-0",
        )}
      >
        {mode.current === "dark" ? "Light mode" : "Dark mode"}
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
