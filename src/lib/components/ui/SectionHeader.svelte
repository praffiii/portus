<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import type { Snippet } from "svelte";

  let {
    id,
    controlsId,
    label,
    count,
    collapsible = false,
    expanded = true,
    ontoggle,
    trailing
  }: {
    id: string;
    controlsId?: string;
    label: string;
    count?: number;
    collapsible?: boolean;
    expanded?: boolean;
    ontoggle?: () => void;
    trailing?: Snippet;
  } = $props();

  const panelId = $derived(controlsId ?? id);
</script>

<div class="section-heading glass-chrome">
  <div class="heading-left">
    {#if collapsible}
      <button
        class="collapse-btn"
        class:expanded
        type="button"
        aria-expanded={expanded}
        aria-controls={panelId}
        onclick={ontoggle}
      >
        <ChevronDown size={13} strokeWidth={2.1} aria-hidden="true" />
      </button>
      <button class="heading-toggle" type="button" aria-expanded={expanded} aria-controls={panelId} onclick={ontoggle}>
        <h2 {id}>{label}</h2>
        {#if count !== undefined}
          <span class="count">{count}</span>
        {/if}
      </button>
    {:else}
      <h2 {id}>{label}</h2>
      {#if count !== undefined}
        <span class="count">{count}</span>
      {/if}
    {/if}
  </div>
  {#if trailing}
    <div class="heading-trailing">
      {@render trailing()}
    </div>
  {/if}
</div>

<style>
  .section-heading {
    position: sticky;
    z-index: 2;
    top: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: var(--section-header-h);
    padding: 0 var(--row-pad-x);
    border-bottom: 1px solid var(--hairline);
    color: var(--text-muted);
  }

  .heading-left {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 6px;
  }

  .heading-toggle {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: none;
    color: inherit;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }

  .heading-toggle:hover h2,
  .heading-toggle:hover .count {
    color: var(--text-secondary);
  }

  .heading-trailing {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 4px;
  }

  h2,
  .count {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .collapse-btn {
    display: grid;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    place-items: center;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--text-muted);
    background: transparent;
    cursor: pointer;
    transition:
      background var(--motion-fast),
      border-color var(--motion-fast);
  }

  .collapse-btn:hover {
    background: var(--surface-pressed);
    border-color: var(--glass-border);
    box-shadow: inset 0 1px 0 var(--glass-specular);
  }

  .collapse-btn.expanded :global(svg) {
    transform: rotate(180deg);
    transition: transform var(--motion-fast);
  }

  .collapse-btn :global(svg) {
    transition: transform var(--motion-fast);
  }

  @media (prefers-reduced-motion: reduce) {
    .collapse-btn,
    .collapse-btn :global(svg) {
      transition: none;
    }
  }
</style>
