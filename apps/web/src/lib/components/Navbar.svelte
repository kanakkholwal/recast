<script lang="ts">
    import Logo from "$lib/logo.svelte";
    import { FloatingMenu } from "$lib/motion-core";

    const menuGroups = [
        {
            title: "Product",
            variant: "default" as const,
            links: [
                { label: "Capabilities", href: "/#workflow" },
                { label: "Use Cases", href: "/#use-cases" },
                {
                    label: "Changelog",
                    href: "https://github.com/kanakkholwal/recast/releases",
                },
            ],
        },
        {
            title: "Resources",
            variant: "muted" as const,
            links: [
                {
                    label: "GitHub",
                    href: "https://github.com/kanakkholwal/recast",
                },
                { label: "Documentation", href: "#" },
            ],
        },
    ];
    let menuContainer: HTMLElement;
</script>

<div
    bind:this={menuContainer}
    class="fixed top-4 left-0 right-0 z-50 flex justify-center"
>
    <FloatingMenu
        portalTarget={menuContainer}
        {menuGroups}
        primaryButton={{ label: "Download", href: "/download" }}
        secondaryButton={{
            label: "GitHub",
            href: "https://github.com/kanakkholwal/recast",
        }}
        classes={{
            root: "shadow-craft-md",
            header: "px-4 py-2",
        }}
    >
        {#snippet logo()}
            <a
                href="/"
                class="flex items-center gap-3 active:scale-95 transition-transform"
                aria-label="Home"
            >
                <div
                    class="size-7 flex items-center justify-center p-0.5 rounded-lg bg-foreground/5 shadow-craft-sm"
                >
                    <Logo size="24" />
                </div>
                <span class="text-base font-bold tracking-tight text-foreground"
                    >Recast</span
                >
            </a>
        {/snippet}
    </FloatingMenu>
</div>
