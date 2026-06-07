<script lang="ts">
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import type { DockerRow } from "$lib/snapshot-adapter";

  let { containers }: { containers: DockerRow[] } = $props();
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
              <span class="ports">:{container.ports.join(", :")}</span>
            {/if}
          </div>
          <div class="secondary" title={`${container.image} · ${container.detail}`}>
            <span class="image">{container.image}</span>
            <span aria-hidden="true">·</span>
            <span class="detail">{container.detail}</span>
          </div>
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
    grid-template-columns: 16px minmax(0, 1fr);
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
    background: var(--surface);
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
    gap: 5px;
    margin-top: 3px;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.25;
    white-space: nowrap;
  }

  .image {
    min-width: 24px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail {
    flex: 0 0 auto;
  }

  @media (prefers-reduced-motion: reduce) {
    li {
      transition: none;
    }
  }
</style>
