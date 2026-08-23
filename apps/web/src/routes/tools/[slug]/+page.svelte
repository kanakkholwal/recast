<script lang="ts">
import ToolPage from "$lib/components/ToolPage.svelte";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();
</script>

<ToolPage tool={data.tool} />
