<script lang="ts">
import { Grid3x3, List } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { ButtonGroup } from "@recast/ui/button-group";

interface Props {
	value: "grid" | "list";
}

let { value = $bindable("grid") }: Props = $props();
</script>

<ButtonGroup>
  <Button
    variant={value === "grid" ? "secondary" : "ghost"}
    size="icon-sm"
    onclick={() => (value = "grid")}
    aria-label="Grid view"
    aria-pressed={value === "grid"}
    title="Grid view"
  >
    <Grid3x3 size={12} />
  </Button>
  <Button
    variant={value === "list" ? "secondary" : "ghost"}
    size="icon-sm"
    onclick={() => (value = "list")}
    aria-label="List view"
    aria-pressed={value === "list"}
    title="List view"
  >
    <List size={12} />
  </Button>
</ButtonGroup>
