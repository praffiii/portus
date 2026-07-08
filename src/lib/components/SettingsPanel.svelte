<script lang="ts">
  import FilterChip from "$lib/components/ui/FilterChip.svelte";
  import { getTheme, setTheme, type Theme } from "$lib/theme";

  let theme = $state<Theme>(getTheme());

  function pick(next: Theme) {
    theme = next;
    setTheme(next);
  }
</script>

<section class="settings" aria-labelledby="settings-heading">
  <h2 id="settings-heading">Settings</h2>

  <div class="setting-row">
    <div class="setting-copy">
      <span class="setting-label">Appearance</span>
      <span class="setting-hint">Light or dark popover theme</span>
    </div>
    <div class="theme-toggle" role="group" aria-label="Appearance">
      <FilterChip label="Light" active={theme === "light"} onclick={() => pick("light")} />
      <FilterChip label="Dark" active={theme === "dark"} onclick={() => pick("dark")} />
    </div>
  </div>
</section>

<style>
  .settings {
    padding: 16px var(--row-pad-x) 20px;
  }

  h2 {
    margin: 0 0 14px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .setting-row {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--hairline);
    border-radius: 8px;
    background: var(--surface);
  }

  .setting-copy {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .setting-label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .setting-hint {
    font-size: 11px;
    color: var(--text-muted);
    line-height: 1.35;
  }

  .theme-toggle {
    display: flex;
    gap: 6px;
  }
</style>
