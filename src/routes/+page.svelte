<main class="popover">
  <header class="glance">
    <div>
      <p class="eyebrow">Portus</p>
      <p class="summary">0 running · 0 waiting?</p>
    </div>
    <button class="icon-button" type="button" aria-label="Settings">⚙</button>
  </header>

  <section aria-labelledby="ports-heading">
    <h2 id="ports-heading">Ports</h2>
    <div class="empty-row">
      <p class="primary">No listening ports yet</p>
      <p class="secondary">Layer 1 will connect the Rust port scanner.</p>
    </div>
  </section>

  <section aria-labelledby="docker-heading">
    <h2 id="docker-heading">Docker</h2>
    <div class="empty-row">
      <p class="primary">Docker not checked yet</p>
      <p class="secondary">The read-only Docker probe is next.</p>
    </div>
  </section>
</main>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    min-height: 100%;
    font-family:
      "Geist",
      -apple-system,
      BlinkMacSystemFont,
      "Segoe UI",
      sans-serif;
    color: #18181b;
    background: #fbfbfd;
    font-synthesis: none;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
  }

  :global(button) {
    font: inherit;
  }

  .popover {
    width: 380px;
    min-height: 520px;
    background: #fbfbfd;
  }

  .glance {
    position: sticky;
    top: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 52px;
    padding: 0 12px;
    border-bottom: 1px solid #e5e7eb;
    background: #ffffff;
  }

  .eyebrow,
  .summary,
  h2,
  .primary,
  .secondary {
    margin: 0;
  }

  .eyebrow {
    font-size: 11px;
    font-weight: 600;
    color: #71717a;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .summary {
    margin-top: 2px;
    font-size: 13px;
    font-weight: 600;
  }

  .icon-button {
    display: grid;
    width: 28px;
    height: 28px;
    place-items: center;
    border: 1px solid #e5e7eb;
    border-radius: 6px;
    color: #18181b;
    background: #ffffff;
    cursor: pointer;
  }

  section {
    border-bottom: 1px solid #e5e7eb;
  }

  h2 {
    position: sticky;
    top: 52px;
    padding: 8px 12px;
    border-bottom: 1px solid #e5e7eb;
    color: #71717a;
    background: #fbfbfd;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .empty-row {
    padding: 12px;
  }

  .primary {
    font-size: 13px;
    font-weight: 500;
  }

  .secondary {
    margin-top: 3px;
    color: #71717a;
    font-size: 11px;
    line-height: 1.35;
  }

  @media (prefers-color-scheme: dark) {
    :global(html),
    :global(body),
    .popover {
      color: #f4f4f5;
      background: #1c1c1e;
    }

    .glance,
    .icon-button {
      color: #f4f4f5;
      border-color: #38383c;
      background: #242427;
    }

    section,
    h2 {
      border-color: #38383c;
    }

    h2 {
      background: #1c1c1e;
    }

    .eyebrow,
    h2,
    .secondary {
      color: #a1a1aa;
    }
  }
</style>
