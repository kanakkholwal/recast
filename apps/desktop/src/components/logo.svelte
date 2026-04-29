<script lang="ts">
  import { onMount } from "svelte";
  type Theme = "light" | "dark" | "system";

  let currentTheme = $state<Theme>("system");
  onMount(() => {
    const storedTheme = localStorage.getItem(
      "mode-watcher-mode",
    ) as Theme | null;
    if (storedTheme) currentTheme = storedTheme;
  });

  let {
    color = currentTheme === "light" ? "white" : "black",
    fill = currentTheme === "light" ? "black" : "white",
    size = "512",
    ...rest
  } = $props();
</script>

<svg
  viewBox="0 0 512 512"
  xmlns="http://www.w3.org/2000/svg"
  {...rest}
  {fill}
  width={size}
  height={size}
>
  <rect width="512" height="512" rx="256" {fill} />
  <rect x="230" y="166" width="60" height="180" rx="30" fill={color} />
  <rect x="111" y="166" width="60" height="180" rx="30" fill={color} />
  <rect x="349" y="166" width="60" height="180" rx="30" fill={color} />
</svg>
