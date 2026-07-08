<script lang="ts">
  let {
    label,
    active = false,
    onclick
  }: {
    label: string;
    active?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<button class="filter-chip" class:active type="button" {onclick}>
  {label}
</button>

<style>
  .filter-chip {
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    color: var(--text-muted);
    background: var(--surface);
    backdrop-filter: var(--glass-blur-chrome);
    -webkit-backdrop-filter: var(--glass-blur-chrome);
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    cursor: pointer;
    transition:
      color var(--motion-fast),
      background var(--motion-fast),
      border-color var(--motion-fast),
      box-shadow var(--motion-fast);
  }

  .filter-chip.active {
    border-color: var(--hairline-strong);
    color: var(--text-primary);
    background: var(--surface-hi);
    box-shadow: inset 0 1px 0 var(--glass-specular);
  }

  .filter-chip:hover:not(.active) {
    background: var(--surface-pressed);
  }

  @media (prefers-reduced-motion: reduce) {
    .filter-chip {
      transition: none;
    }
  }
</style>
