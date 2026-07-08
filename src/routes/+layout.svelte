<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { initTheme, watchSystemTheme } from "$lib/theme";

  onMount(() => {
    initTheme();
    return watchSystemTheme();
  });
</script>

<slot />
