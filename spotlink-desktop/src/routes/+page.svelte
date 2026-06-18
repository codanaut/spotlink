<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  // Explicit type matching our Rust SpotLinkMatch struct
  interface SpotLinkMatch {
    callsign: string;
    band: string;
    mode: string;
    sent_snr: number;
    recv_snr: number;
    timestamp: string;
  }

  interface EngineStats {
    outgoing: number;
    incoming: number;
    matches: number;
  }

  // State Management with strict type assignments
  let targetCallsign: string = "";
  let activeTrackingCallsign: string = "";
  let filterQuery: string = "";
  let matches: SpotLinkMatch[] = [];

  let unlistenMatch: (() => void) | null = null;
  let unlistenClear: (() => void) | null = null;
  let pruneInterval: ReturnType<typeof setInterval> | null = null;

  let currentStats: EngineStats = { outgoing: 0, incoming: 0, matches: 0 };
  let unlistenStats: (() => void) | null = null;

  onMount(async () => {
    // Listen for new matches or updates to existing active paths
    unlistenMatch = await listen<SpotLinkMatch>("new-match", (event) => {
      const newMatch = event.payload;

      // Check if this station already has an open card displayed
      const existingIdx = matches.findIndex(
        (m) => m.callsign === newMatch.callsign,
      );

      if (existingIdx !== -1) {
        // Update the existing card layout in-place with fresh signal metrics
        matches[existingIdx] = newMatch;
        matches = [...matches];
      } else {
        // Prepend brand-new station paths directly to the top of the feed
        matches = [newMatch, ...matches];
      }
    });

    // Listen for global engine reset signals (total inactivity)
    unlistenClear = await listen<void>("clear-matches", () => {
      matches = [];
    });

    // Listen for the live stats coming from the Rust engine
    unlistenStats = await listen<EngineStats>("stats-update", (event) => {
      currentStats = event.payload;
    });

    // Frontend Pruning: Run a sweep every 3 seconds to clear matches older than 5 minutes
    pruneInterval = setInterval(() => {
      const now = Date.now();
      const fiveMinutesInMs = 300000; // 5 mins * 60 secs * 1000 ms

      matches = matches.filter((m) => {
        const matchTime = new Date(m.timestamp).getTime();
        return now - matchTime < fiveMinutesInMs;
      });
    }, 3000);
  });

  // Cleanup listeners when the component is destroyed
  onDestroy(() => {
    if (unlistenMatch) unlistenMatch();
    if (unlistenClear) unlistenClear();
    if (pruneInterval) clearInterval(pruneInterval);
    if (unlistenStats) unlistenStats();
  });

  // Actions
  async function handleStartTracking(): Promise<void> {
    if (!targetCallsign.trim()) return;
    try {
      const formattedCall = targetCallsign.trim().toUpperCase();
      await invoke("track_callsign", { callsign: formattedCall });
      activeTrackingCallsign = formattedCall;
    } catch (err) {
      console.error("Failed to start tracking:", err);
    }
  }

  // Stop tracking callsign
  async function handleStopTracking(): Promise<void> {
    try {
      await invoke("stop_tracking");
      activeTrackingCallsign = "";
      matches = [];
    } catch (err) {
      console.error("Failed to stop tracking:", err);
    }
  }

  // Reactive store filtering based on search parameters
  $: filteredMatches = matches.filter((m: SpotLinkMatch) =>
    m.callsign.toUpperCase().includes(filterQuery.toUpperCase()),
  );

  // Map SNR boundaries to style color elements safely
  function getSnrClass(val: number): string {
    if (val >= -10) return "snr-green";
    if (val >= -15) return "snr-yellow";
    if (val >= -19) return "snr-orange";
    return "snr-red";
  }
</script>

<main class="container">
  <header class="control-panel">
    <h1 class="title">SpotLink</h1>

    {#if !activeTrackingCallsign}
      <div class="input-group">
        <input
          type="text"
          placeholder=" "
          bind:value={targetCallsign}
          on:keydown={(e) => e.key === "Enter" && handleStartTracking()}
        />
        <button on:click={handleStartTracking} class="btn-start"
          >Track Stream</button
        >
      </div>
    {:else}
      <div class="active-group">
        <div class="spacer"></div>

        <div class="status-indicator">
          Listening to: <span class="active-call">{activeTrackingCallsign}</span
          >
        </div>

        <div class="button-wrapper">
          <button on:click={handleStopTracking} class="btn-stop">Stop</button>
        </div>
      </div>
    {/if}
  </header>

  {#if activeTrackingCallsign}
    <section class="stats-panel">
      <div class="stat-box">
        <span class="stat-label">Outgoing Signals</span>
        <span class="stat-value">{currentStats.outgoing}</span>
      </div>
      <div class="stat-box">
        <span class="stat-label">Incoming Signals</span>
        <span class="stat-value">{currentStats.incoming}</span>
      </div>
      <div class="stat-box">
        <span class="stat-label">Total Matches</span>
        <span class="stat-value">{currentStats.matches}</span>
      </div>
    </section>
  {/if}

  {#if matches.length > 0 || filterQuery}
    <section class="filter-panel">
      <input
        type="text"
        placeholder="🔍 Filter matches"
        bind:value={filterQuery}
      />
    </section>
  {/if}

  <section class="feed">
    {#if filteredMatches.length === 0}
      <div class="empty-state">
        {activeTrackingCallsign
          ? "Awaiting incoming bi-directional matches..."
          : "Configure a callsign above to begin."}
      </div>
    {:else}
      {#each filteredMatches as match}
        <div class="match-card">
          <div class="card-header">
            <span class="callsign-badge">
              {match.callsign}
              {#if match.sent_snr >= -10 && match.recv_snr >= -10}
                <span class="star">⭐</span>
              {/if}
            </span>
            <span class="timestamp"
              >{new Date(match.timestamp).toLocaleTimeString()}</span
            >
          </div>
          <div class="card-body">
            <div class="meta-info">
              <span class="badge">{match.band}</span>
              <span class="badge">{match.mode}</span>
            </div>
            <div class="snr-display">
              <span class="snr-metric"
                >Sent: <strong class={getSnrClass(match.sent_snr)}
                  >{match.sent_snr} dB</strong
                ></span
              >
              <span class="divider">|</span>
              <span class="snr-metric"
                >Recv: <strong class={getSnrClass(match.recv_snr)}
                  >{match.recv_snr} dB</strong
                ></span
              >
            </div>
          </div>
        </div>
      {/each}
    {/if}
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
      Helvetica, sans-serif;
    background-color: #121214;
    color: #e1e1e6;
  }

  .container {
    max-width: 800px;
    margin: 0 auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 15px;
  }

  .control-panel,
  .filter-panel {
    background: #1e1e24;
    padding: 15px 20px;
    border-radius: 8px;
    border: 1px solid #29292e;
  }

  .title {
    text-align: center;
    margin: 0 0 20px 0;
    font-size: 2rem;
    color: #63b1fc;
  }

  .input-group {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  input {
    flex: 1;
    background: #121214;
    border: 1px solid #29292e;
    color: white;
    padding: 10px;
    border-radius: 4px;
    font-size: 1rem;
    text-transform: uppercase;
  }

  input:focus {
    outline: 1px solid #00b37e;
  }

  button {
    padding: 10px 20px;
    border: none;
    border-radius: 4px;
    font-weight: bold;
    cursor: pointer;
    font-size: 1rem;
  }

  .btn-start {
    background-color: #00b37e;
    color: white;
  }
  .btn-stop {
    background-color: #f75a68;
    color: white;
  }

  /* --- The Active State Layout --- */
  .active-group {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .spacer {
    flex: 1; /* Takes up exactly 1 unit of space on the left */
  }

  .status-indicator {
    text-align: center;
    font-size: 1.1rem;
    white-space: nowrap; /* Prevents the text from wrapping weirdly */
  }

  .button-wrapper {
    flex: 1; /* Takes up exactly 1 unit of space on the right */
    display: flex;
    justify-content: flex-end; /* Shoves the button to the far right edge */
  }
  /* ------------------------------- */
  .active-call {
    color: #63b1fc;
    font-weight: bold;
  }

  .feed {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .match-card {
    background: #1e1e24;
    border: 1px solid #29292e;
    border-radius: 6px;
    padding: 12px 16px;
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .callsign-badge {
    font-size: 1.2rem;
    font-weight: bold;
    color: #63b1fc;
  }

  .timestamp {
    color: #7c7c8a;
    font-size: 0.9rem;
  }

  .card-body {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .meta-info {
    display: flex;
    gap: 6px;
  }
  .badge {
    background: #29292e;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 0.85rem;
  }

  .snr-display {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .divider {
    color: #29292e;
  }

  .snr-green {
    color: #00b37e;
  }
  .snr-yellow {
    color: #fba94c;
  }
  .snr-orange {
    color: #e77c22;
  }
  .snr-red {
    color: #f75a68;
  }

  .empty-state {
    text-align: center;
    padding: 4px 0;
    color: #7c7c8a;
    font-style: italic;
  }
  .star {
    margin-left: 4px;
  }

  .stats-panel {
    display: flex;
    justify-content: space-around;
    background: #1e1e24;
    padding: 15px;
    border-radius: 8px;
    border: 1px solid #29292e;
  }

  .stat-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
  }

  .stat-label {
    font-size: 0.9rem;
    color: #7c7c8a;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .stat-value {
    font-size: 1.8rem;
    font-weight: bold;
    color: #63b1fc;
  }
</style>
