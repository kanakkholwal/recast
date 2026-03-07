<script lang="ts">
	import "../app.css";
	import "@fontsource-variable/google-sans";
	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" type="image/png" href="/favicon.ico" />
</svelte:head>

{@render children()}
