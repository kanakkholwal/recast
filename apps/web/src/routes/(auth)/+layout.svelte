<script lang="ts">
import { ArrowLeft } from "@recast/icons";

let { children } = $props();
</script>

<div class="relative min-h-screen text-foreground">
	<a
		href="/"
		class="absolute left-6 top-6 z-20 inline-flex items-center gap-1.5 text-body-sm text-muted-foreground underline-offset-4 transition-colors hover:text-foreground hover:underline"
	>
		<ArrowLeft class="size-3.5" />
		Back to site
	</a>

	<!-- min-h-screen + grid keeps the card centred whether it is short (login)
	     or tall (verify-email). -->
	<div class="grid min-h-screen place-items-center px-6 py-20">
		{@render children()}
	</div>
</div>
