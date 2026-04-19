<script lang="ts">
	import { Container } from "$lib/components";
	import Logo from "$lib/logo.svelte";
</script>

<footer class="py-24 border-t border-border-low bg-muted/20 dark:bg-[#0a0a0a]">
	<Container>
		<div class="grid grid-cols-1 md:grid-cols-4 gap-12 md:gap-8 mb-20">
			<!-- Branding -->
			<div class="col-span-1 md:col-span-2 space-y-6">
				<div class="flex items-center gap-3">
					<Logo size="32" />
					<span class="text-xl font-bold tracking-tight text-foreground">Recast</span>
				</div>
				<p class="text-base text-foreground/50 max-w-sm font-medium leading-relaxed">
					The intentional screen recorder. Free for local use. No sign-up required. Start creating cinematic, refined screen recordings in seconds.
				</p>
			</div>

			<!-- Links -->
			<div class="space-y-6">
				<h4 class="text-sm font-bold text-foreground uppercase tracking-widest">Product</h4>
				<ul class="space-y-4">
					<li><a href="/download" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors">Download</a></li>
					<li><a href="/#features" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors">Features</a></li>
					<li><a href="/#how-it-works" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors">Workflow</a></li>
				</ul>
			</div>

			<div class="space-y-6">
				<h4 class="text-sm font-bold text-foreground uppercase tracking-widest">Connect</h4>
				<ul class="space-y-4">
					<li><a href="https://github.com/kanakkholwal/recast" target="_blank" rel="noopener noreferrer" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors">GitHub</a></li>
					<li><a href="https://x.com/kanakkholwal" target="_blank" rel="noopener noreferrer" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors">X (Twitter)</a></li>
					<li><a href="mailto:recast@nexonauts.com" class="text-[15px] font-medium text-foreground/60 hover:text-foreground transition-colors">Contact</a></li>
				</ul>
			</div>
		</div>

		<!-- Bottom Bar -->
		<div class="pt-8 border-t border-border-low flex flex-col md:flex-row items-center justify-between gap-6 font-medium">
			<p class="text-xs text-foreground/40 font-bold uppercase tracking-widest">
				Built for clarity. Designed for speed.
			</p>
			<p class="text-[13px] text-foreground/40">
				© {new Date().getFullYear()} Recast. All rights reserved.
			</p>
		</div>
	</Container>
</footer>