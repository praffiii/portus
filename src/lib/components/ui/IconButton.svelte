<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    label,
    title,
    disabled = false,
    muted = false,
    active = false,
    onclick,
    children
  }: {
    label: string;
    title?: string;
    disabled?: boolean;
    muted?: boolean;
    active?: boolean;
    onclick?: () => void;
    children: Snippet;
  } = $props();
</script>

<button
  class="icon-button glass-pill"
  class:muted
  class:active
  type="button"
  aria-label={label}
  {title}
  {disabled}
  {onclick}
>
  {@render children()}
</button>

<style>
  .icon-button {
    display: grid;
    width: 28px;
    height: 28px;
    flex: 0 0 28px;
    place-items: center;
    padding: 0;
    border-radius: 8px;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0.92;
    transition:
      opacity var(--motion-fast),
      background var(--motion-fast),
      box-shadow var(--motion-fast);
  }

  .icon-button.muted {
    cursor: default;
    opacity: 0.5;
  }

  .icon-button.active {
    opacity: 1;
    color: var(--text-primary);
    background: var(--surface-hi);
    box-shadow: inset 0 1px 0 var(--glass-specular);
  }

  .icon-button:hover:not(:disabled) {
    opacity: 1;
    background: var(--surface-pressed);
    box-shadow: inset 0 1px 0 var(--glass-specular);
  }

  @media (prefers-reduced-motion: reduce) {
    .icon-button {
      transition: none;
    }
  }
</style>
