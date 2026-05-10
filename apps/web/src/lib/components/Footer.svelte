<script lang="ts">
	import { Container } from "$lib/components";
	import Logo from "$lib/logo.svelte";
	import Github from "@lucide/svelte/icons/github";
	import Mail from "@lucide/svelte/icons/mail";
	import Twitter from "@lucide/svelte/icons/twitter";

	type Link = { label: string; href: string; external?: boolean };

	const cols: { title: string; links: Link[] }[] = [
		{
			title: "Product",
			links: [
				{ label: "Features", href: "/features" },
				{ label: "Download", href: "/download" },
				{ label: "Changelog", href: "/changelog" },
				{ label: "Editor", href: "/#editor" },
			],
		},
		{
			title: "Resources",
			links: [
				{ label: "Documentation", href: "#" },
				{
					label: "GitHub",
					href: "https://github.com/kanakkholwal/recast",
					external: true,
				},
				{
					label: "Releases",
					href: "https://github.com/kanakkholwal/recast/releases",
					external: true,
				},
			],
		},
		{
			title: "Company",
			links: [
				{ label: "Contact", href: "mailto:recast@nexonauts.com" },
				{
					label: "X / Twitter",
					href: "https://x.com/kanakkholwal",
					external: true,
				},
				{
					label: "Discord",
					href: "https://discord.gg/rBCuqRsb5",
					external: true,
				},
			],
		},
	];

	const socials = [
		{ icon: Github, href: "https://github.com/kanakkholwal/recast", label: "GitHub" },
		{ icon: Twitter, href: "https://x.com/kanakkholwal", label: "X / Twitter" },
		{ icon: Mail, href: "mailto:recast@nexonauts.com", label: "Email" },
	];
</script>

<footer class="relative border-t border-border-low/70">
	<div class="bg-aurora absolute inset-x-0 top-0 h-px"></div>
	<Container class="py-20 md:py-28" as="div">
		<div class="grid gap-14 md:grid-cols-12">
			<div class="md:col-span-5">
				<a href="/" class="inline-flex items-center gap-2.5">
					<span class="grid size-8 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm">
						<Logo size="22" color="transparent" fill="currentColor" />
					</span>
					<span class="text-lg font-semibold tracking-tight text-foreground">
						Recast
					</span>
				</a>
				<p class="mt-6 max-w-sm text-pretty text-sm leading-relaxed text-muted-foreground">
					The native screen recorder for founders, indie hackers, and product
					engineers who'd rather ship than open a timeline.
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
						<h4 class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
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

		<div class="mt-20 flex flex-col items-start justify-between gap-4 border-t border-border-low/70 pt-8 md:flex-row md:items-center">
			<p class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
				Skip the editor · Ship the demo
			</p>
			<p class="text-xs text-muted-foreground">
				© {new Date().getFullYear()} Recast. All rights reserved.
			</p>
		</div>
	</Container>
</footer>
