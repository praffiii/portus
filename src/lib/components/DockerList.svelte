<script lang="ts">
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import type { DockerRowView } from "$lib/snapshot-adapter";

  let { containers }: { containers: DockerRowView[] } = $props();
</script>

<section aria-labelledby="docker-heading">
  <div class="section-heading">
    <h2 id="docker-heading">Docker</h2>
    <span>{containers.length}</span>
  </div>

  <ul>
    {#each containers as container (container.name)}
      <li>
        <StatusBadge status={container.status} />
        <div class="details">
          <div class="primary">
            <span class="name" title={container.name}>{container.name}</span>
            {#if container.ports.length > 0}
              <span class:ports-muted={container.status !== "running"} class="ports">:{container.ports.join(", :")}</span>
            {/if}
          </div>
          <div class="secondary" title={`${container.image} · ${container.detail}`}>
            <span class="image">{container.image}</span>
            <span class="sec-dot" aria-hidden="true">·</span>
            <span class="detail">{container.detail}</span>
          </div>
        </div>
        <!-- Presentational until container start/stop commands are wired. -->
        <div class="row-actions">
          {#if container.status === "running"}
            <button class="act-btn kill" type="button" title="Stop container (unavailable)" aria-label={`Stop ${container.name} (unavailable)`} disabled>
              <svg width="9" height="9" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
                <rect x="1.5" y="1.5" width="9" height="9" rx="1.5" />
              </svg>
            </button>
          {:else}
            <button class="act-btn start" type="button" title="Start container (unavailable)" aria-label={`Start ${container.name} (unavailable)`} disabled>
              <svg width="9" height="9" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
                <path d="M2.5 1.5 L10.5 6 L2.5 10.5 Z" />
              </svg>
            </button>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
</section>

<style>
  section {
    border-bottom: 1px solid var(--hairline);
  }

  .section-heading {
    position: sticky;
    z-index: 2;
    top: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 12px;
    border-bottom: 1px solid var(--hairline);
    color: var(--text-muted);
    background: var(--app-bg);
  }

  h2,
  .section-heading span {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  ul {
    padding: 0;
    margin: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    min-height: 54px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--hairline);
    transition: background-color 100ms ease;
  }

  li:last-child {
    border-bottom: 0;
  }

  li:hover {
    background: var(--surface-hi);
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    opacity: 0;
    transition: opacity 100ms ease;
  }

  li:hover .row-actions,
  li:focus-within .row-actions {
    opacity: 1;
  }

  .act-btn {
    display: grid;
    width: 26px;
    height: 26px;
    place-items: center;
    border: 1px solid var(--hairline);
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
    cursor: default;
    transition:
      color 100ms ease,
      background 100ms ease,
      border-color 100ms ease;
  }

  .act-btn.start {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .act-btn:disabled {
    opacity: 0.7;
  }

  .ports-muted {
    color: var(--stopped);
  }

  .sec-dot {
    flex: 0 0 auto;
    color: var(--text-muted);
    opacity: 0.7;
    user-select: none;
  }

  .details {
    min-width: 0;
  }

  .primary,
  .secondary {
    display: flex;
    min-width: 0;
    align-items: baseline;
  }

  .primary {
    gap: 8px;
  }

  .name {
    min-width: 0;
    overflow: hidden;
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ports {
    flex: 0 0 auto;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .secondary {
    gap: 4px;
    margin-top: 3px;
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.25;
    white-space: nowrap;
  }

  .image {
    min-width: 20px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail {
    flex: 0 0 auto;
  }

  @media (prefers-reduced-motion: reduce) {
    li,
    .row-actions,
    .act-btn {
      transition: none;
    }
  }
</style>
