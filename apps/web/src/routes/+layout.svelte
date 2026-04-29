<script lang="ts">
	import { Navbar } from "$lib/components";
	import { Toaster } from "@recast/ui/sonner";
	import { ModeWatcher } from "@recast/ui/theme";
	import "../app.css";

	let { children } = $props();
</script>

<ModeWatcher />
<Navbar />

<div class="relative isolate flex min-h-screen flex-col overflow-x-hidden">
	{@render children()}
</div>

<Toaster position="bottom-right" duration={5000} />
