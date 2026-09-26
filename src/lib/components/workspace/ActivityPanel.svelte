<script lang="ts">
  /**
   * Ops at a glance: firing alerts, what's scheduled next (with Run now) and
   * the recent activity feed. Runs go through the same guarded path as a
   * triggered run (`run_rule_now`), including approval checks.
   */
  import { call, errorMessage } from '$lib/api';
  import { ago } from '$lib/format';
  import type { SystemControlData } from '$lib/types';

  let {
    active = true,
    refreshKey = 0,
    notify,
    onOpenRules
  }: {
    active?: boolean;
    /** Bumped by the page on every `ops://event` to refresh immediately. */
    refreshKey?: number;
    notify: (msg: string, type?: 'info' | 'success' | 'error') => void;
    onOpenRules: () => void;
  } = $props();

  let ops = $state<SystemControlData | null>(null);
  let error = $state('');

  let firing = $derived(ops?.alerts.filter((a) => a.alert.enabled && a.alert.state.firing) ?? []);
  let upcoming = $derived(
    (ops?.tasks ?? [])
      .filter((t) => t.task.enabled)
      .sort((a, b) => String(a.task.next_run ?? '~').localeCompare(String(b.task.next_run ?? '~')))
      .slice(0, 5)
  );
  let automations = $derived((ops?.automations ?? []).filter((a) => a.automation.enabled).slice(0, 5));

  async function load() {
    try {
      ops = await call<SystemControlData>('get_system_control_data');
      error = '';
    } catch (e) {
      error = errorMessage(e);
    }
  }

  $effect(() => {
    void refreshKey;
    if (!active) return;
    load();
    const timer = setInterval(load, 10_000);
    return () => clearInterval(timer);
  });

  async function runNow(kind: 'task' | 'automation', id: string, name: string) {
    try {
      await call('run_rule_now', { ruleKind: kind, id });
      notify(`Running “${name}”… the result shows up here and as a notification`, 'info');
    } catch (e) {
      notify(errorMessage(e), 'error');
    }
  }
</script>

<div class="space-y-3 text-sm">
  {#if error && !ops}
    <p class="text-xs text-red-300">{error}</p>
  {:else if !ops}
    <p class="text-xs text-gray-400">Loading…</p>
  {:else}
    <section>
      <h4 class="head">Alerts</h4>
      {#if firing.length}
        <ul class="space-y-1">
          {#each firing as a (a.alert.id)}
            <li class="row border-red-400/40 bg-red-500/10">
              <span>🚨</span>
              <span class="flex-1 min-w-0"><span class="font-medium">{a.alert.name}</span><span class="block text-[11px] text-gray-400 truncate">{a.description}{a.value != null ? ` · now ${a.value.toFixed(1)}${a.unit}` : ''}</span></span>
              <span class="text-[11px] text-gray-400">{ago(a.alert.state.since)}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="text-xs text-green-300">✓ {ops.alerts.filter((a) => a.alert.enabled).length} alert{ops.alerts.length === 1 ? '' : 's'} watching, none firing</p>
      {/if}
    </section>

    <section>
      <h4 class="head">Scheduled</h4>
      {#if upcoming.length}
        <ul class="space-y-1">
          {#each upcoming as t (t.task.id)}
            <li class="row">
              <span>⏰</span>
              <span class="flex-1 min-w-0"><span class="block truncate">{t.task.name}</span><span class="block text-[11px] text-gray-400 truncate">{t.when} · next {ago(t.task.next_run)}</span></span>
              <button class="mini" onclick={() => runNow('task', t.task.id, t.task.name)} title="Run now (approval rules still apply)">▶</button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="text-xs text-gray-500">No scheduled tasks.</p>
      {/if}
    </section>

    {#if automations.length}
      <section>
        <h4 class="head">Automations</h4>
        <ul class="space-y-1">
          {#each automations as a (a.automation.id)}
            <li class="row">
              <span>🤖</span>
              <span class="flex-1 min-w-0"><span class="block truncate">{a.automation.name}</span><span class="block text-[11px] text-gray-400 truncate">{a.trigger}</span></span>
              <button class="mini" onclick={() => runNow('automation', a.automation.id, a.automation.name)} title="Run now (approval rules still apply)">▶</button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section>
      <h4 class="head">Recent activity</h4>
      {#if ops.activity.length}
        <ul class="space-y-1">
          {#each ops.activity.slice(0, 12) as ev, i (ev.ts + i)}
            <li class="flex gap-2 text-xs">
              <span class={ev.ok ? 'text-green-400' : 'text-red-400'}>{ev.ok ? '✓' : '✗'}</span>
              <!-- Summaries can include command output: plain text only. -->
              <span class="flex-1 min-w-0"><span class="text-gray-200">{ev.name}</span> <span class="text-gray-400 break-words">{ev.summary}</span></span>
              <span class="text-gray-500 shrink-0">{ago(ev.ts)}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="text-xs text-gray-500">Nothing has run yet.</p>
      {/if}
    </section>

    <button class="text-xs text-cosmic-cyan hover:underline" onclick={onOpenRules}>Manage alerts, automations and schedules →</button>
  {/if}
</div>

<style>
  .head {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: rgb(156 163 175);
    margin-bottom: 0.35rem;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.55rem;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
  }
  .mini {
    padding: 0.1rem 0.45rem;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.06);
    font-size: 0.75rem;
  }
  .mini:hover {
    background: rgba(255, 255, 255, 0.14);
  }
</style>
