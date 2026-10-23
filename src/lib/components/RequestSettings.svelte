<script lang="ts">
  import type { RequestSettings } from '../types'
  import { toInt } from '../util/format'
  let { settings }: { settings: RequestSettings } = $props()
</script>

<div class="settings">
  <label class="field">
    <span>Timeout (ms, 0 = none)</span>
    <input class="input" type="number" min="0" step="1000" value={settings.timeoutMs} oninput={(e) => (settings.timeoutMs = toInt(e.currentTarget.value, 0))} />
  </label>
  <label class="check">
    <input type="checkbox" bind:checked={settings.followRedirects} /> Follow redirects
  </label>
  <label class="check">
    <input type="checkbox" bind:checked={settings.verifySsl} /> Verify SSL certificates
    <span class="faint">— turn off for local self-signed certificates</span>
  </label>
</div>

<style>
  .settings { padding: 14px; display: flex; flex-direction: column; gap: 12px; max-width: 480px; }
  .field { margin: 0; }
  .field .input { width: 160px; }
</style>
