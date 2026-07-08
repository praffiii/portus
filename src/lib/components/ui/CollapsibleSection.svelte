<script lang="ts">
  import type { Snippet } from "svelte";
  import SectionHeader from "$lib/components/ui/SectionHeader.svelte";

  const STORAGE_PREFIX = "portus:section:";

  let {
    id,
    label,
    count,
    defaultExpanded = true,
    persist = true,
    children
  }: {
    id: string;
    label: string;
    count?: number;
    defaultExpanded?: boolean;
    persist?: boolean;
    children: Snippet;
  } = $props();

  function readExpanded(): boolean {
    if (!persist || typeof localStorage === "undefined") return defaultExpanded;
    const stored = localStorage.getItem(`${STORAGE_PREFIX}${id}`);
    if (stored === null) return defaultExpanded;
    return stored === "true";
  }

  let expanded = $state(readExpanded());

  function toggle() {
    expanded = !expanded;
    if (persist && typeof localStorage !== "undefined") {
      localStorage.setItem(`${STORAGE_PREFIX}${id}`, String(expanded));
    }
  }

  export function expand() {
    expanded = true;
    if (persist && typeof localStorage !== "undefined") {
      localStorage.setItem(`${STORAGE_PREFIX}${id}`, "true");
    }
  }

  export function collapse() {
    expanded = false;
    if (persist && typeof localStorage !== "undefined") {
      localStorage.setItem(`${STORAGE_PREFIX}${id}`, "false");
    }
  }
</script>

<section class="list-section" aria-labelledby="{id}-heading">
  <SectionHeader
    id="{id}-heading"
    {label}
    {count}
    collapsible
    {expanded}
    ontoggle={toggle}
  />
  {#if expanded}
    <div id={id}>
      {@render children()}
    </div>
  {/if}
</section>
