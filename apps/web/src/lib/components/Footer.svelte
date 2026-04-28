<script lang="ts">
	import { Container } from "$lib/components";
	import Logo from "$lib/logo.svelte";
</script>

<footer class="py-20 md:py-32 border-t border-border-low bg-background relative overflow-hidden">
    <div class="absolute inset-0 bg-linear-to-b from-transparent to-muted/20 pointer-events-none"></div>
	<Container class="relative z-10">
		<div class="grid grid-cols-1 md:grid-cols-4 gap-12 md:gap-8 mb-24">
			<!-- Branding -->
			<div class="col-span-1 md:col-span-2 space-y-6 lg:pr-12">
				<div class="flex items-center gap-3">
                    <div class="size-8 flex items-center justify-center p-1 rounded-xl bg-muted border border-border shadow-craft-sm">
					    <Logo size="24" />
                    </div>
					<span class="text-xl font-bold tracking-tight text-foreground">Recast</span>
				</div>
				<p class="text-base text-foreground/50 max-w-sm font-medium leading-relaxed">
					The intentional screen recorder. Free during beta. No sign-up required. Start creating cinematic, refined screen recordings in seconds.
				</p>
			</div>

			<!-- Links -->
			<div class="space-y-6">
				<h4 class="text-sm font-bold text-foreground uppercase tracking-widest">Product</h4>
				<ul class="space-y-4">
					<li><a href="/download" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors inline-block hover:translate-x-1 duration-300">Download</a></li>
					<li><a href="/#workflow" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors inline-block hover:translate-x-1 duration-300">Capabilities</a></li>
					<li><a href="/#use-cases" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors inline-block hover:translate-x-1 duration-300">Use Cases</a></li>
				</ul>
			</div>

			<div class="space-y-6">
				<h4 class="text-sm font-bold text-foreground uppercase tracking-widest">Connect</h4>
				<ul class="space-y-4">
					<li><a href="https://github.com/kanakkholwal/recast" target="_blank" rel="noopener noreferrer" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors inline-block hover:translate-x-1 duration-300">GitHub</a></li>
					<li><a href="https://x.com/kanakkholwal" target="_blank" rel="noopener noreferrer" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors inline-block hover:translate-x-1 duration-300">X (Twitter)</a></li>
					<li><a href="mailto:recast@nexonauts.com" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors inline-block hover:translate-x-1 duration-300">Contact</a></li>
				</ul>
			</div>
		</div>

		<!-- Bottom Bar -->
		<div class="pt-8 border-t border-border-low/50 flex flex-col md:flex-row items-center justify-between gap-6 font-medium">
			<p class="text-xs text-foreground/30 font-bold uppercase tracking-widest">
				Built for clarity. Designed for speed.
			</p>
			<p class="text-[13px] text-foreground/40">
				© {new Date().getFullYear()} Recast. All rights reserved.
			</p>
		</div>
	</Container>
</footer>