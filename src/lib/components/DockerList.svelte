<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import LogPeek from "$lib/components/LogPeek.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import SectionHeader from "$lib/components/ui/SectionHeader.svelte";
  import type { DockerRowView } from "$lib/snapshot-adapter";

  let { containers }: { containers: DockerRowView[] } = $props();
  let expanded: Record<string, boolean> = $state({});

  function toggle(id: string) {
    expanded = { ...expanded, [id]: !expanded[id] };
  }
</script>

{#if containers.length > 0}
  <section class="list-section" aria-labelledby="docker-heading">
    <SectionHeader id="docker-heading" label="Docker" count={containers.length} />

    <ul>
      {#each containers as container (container.name)}
        <li class="list-row">
          <div class="row-grid">
            <StatusBadge status={container.status} />
            <div class="details">
              <div class="row-primary">
                <span class="row-title" title={container.name}>{container.name}</span>
                {#if container.ports.length > 0}
                  <span class:ports-muted={container.status !== "running"} class="ports">:{container.ports.join(", :")}</span>
                {/if}
              </div>
              <div class="row-secondary" title={`${container.image} · ${container.detail}`}>
                <span class="image">{container.image}</span>
                <span class="sec-dot" aria-hidden="true">·</span>
                <span class="detail">{container.detail}</span>
              </div>
            </div>
            <div class="row-actions">
              {#if container.status === "running"}
                <button
                  class="act-btn"
                  class:expanded={expanded[container.id]}
                  type="button"
                  title={`${expanded[container.id] ? "Hide" : "Show"} output for ${container.name}`}
                  aria-label={`${expanded[container.id] ? "Hide" : "Show"} output for ${container.name}`}
                  onclick={() => toggle(container.id)}
                >
                  <ChevronDown size={13} strokeWidth={2.1} aria-hidden="true" />
                </button>
              {/if}
            </div>
          </div>
          {#if container.status === "running" && expanded[container.id]}
            <LogPeek containerId={container.id} readonly />
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .ports {
    flex: 0 0 auto;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .ports-muted {
    color: var(--stopped);
  }

  .image {
    min-width: 20px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail {
    flex: 0 0 auto;
  }
</style>
