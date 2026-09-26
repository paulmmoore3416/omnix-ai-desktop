<script lang="ts">
  import { call, errorMessage } from '$lib/api';
  import { ago } from '$lib/format';
  import type { Condition, OpsAction, SystemControlData, Trigger } from '$lib/types';
  import ActionEditor from './ActionEditor.svelte';
  import ConditionEditor from './ConditionEditor.svelte';

  let {
    kind,
    data,
    reload,
    onError,
    onInfo
  }: {
    kind: 'alerts' | 'automations' | 'tasks';
    data: SystemControlData | null;
    reload: () => Promise<void>;
    onError: (msg: string) => void;
    onInfo: (msg: string) => void;
  } = $props();

  let busy = $state(false);
  let showForm = $state(false);

  // --- forms
  const blankCondition = (): Condition => ({ metric: 'cpu', op: 'above', threshold: 90, sustain_secs: 120, target: '' });
  let alertName = $state('');
  let alertCond = $state<Condition>(blankCondition());
  let alertNotify = $state(true);

  let autoName = $state('');
  let triggerKind = $state<Trigger['kind']>('condition');
  let trigCond = $state<Condition>(blankCondition());
  let trigAlert = $state('');
  let trigProcess = $state('');
  let trigPath = $state('');
  let idleMinutes = $state(30);
  let idleCpu = $state(5);
  let autoAction = $state<OpsAction>({ kind: 'notify', title: '', message: '' });

  let taskName = $state('');
  let schedule = $state('0 8 * * 1-5');
  let taskAction = $state<OpsAction>({ kind: 'ai_report', prompt: 'Summarise the health of this computer and anything that needs my attention.', save_to_memory: false });

  const schedules = [
    { v: '@every 15m', l: 'Every 15 minutes' },
    { v: '@hourly', l: 'Every hour' },
    { v: '0 8 * * 1-5', l: 'Weekdays at 08:00' },
    { v: '0 9 * * *', l: 'Every day at 09:00' },
    { v: '0 2 * * *', l: 'Every night at 02:00' },
    { v: '0 3 * * 0', l: 'Sundays at 03:00' }
  ];

  const clean = (c: Condition): Condition => ({ ...c, target: c.target?.trim() ? c.target.trim() : null });

  function trigger(): Trigger {
    switch (triggerKind) {
      case 'alert': return { kind: 'alert', alert_id: trigAlert };
      case 'process_start': return { kind: 'process_start', name: trigProcess.trim() };
      case 'process_stop': return { kind: 'process_stop', name: trigProcess.trim() };
      case 'file_change': return { kind: 'file_change', path: trigPath.trim() };
      case 'idle': return { kind: 'idle', minutes: Number(idleMinutes), cpu_below: Number(idleCpu) };
      default: return { kind: 'condition', condition: clean(trigCond) };
    }
  }

  async function run(fn: () => Promise<unknown>, done: string) {
    busy = true;
    try {
      await fn();
      onInfo(done);
      await reload();
      return true;
    } catch (e) {
      onError(errorMessage(e));
      return false;
    } finally {
      busy = false;
    }
  }

  async function create() {
    let ok = false;
    if (kind === 'alerts') {
      ok = await run(() => call('create_alert', { input: { name: alertName, condition: clean(alertCond), notify: alertNotify } }), `Alert “${alertName}” created`);
      if (ok) { alertName = ''; alertCond = blankCondition(); }
    } else if (kind === 'automations') {
      ok = await run(() => call('create_automation', { input: { name: autoName, trigger: trigger(), action: autoAction } }), `Automation “${autoName}” created`);
      if (ok) autoName = '';
    } else {
      ok = await run(() => call('create_scheduled_task', { input: { name: taskName, schedule, action: taskAction } }), `Task “${taskName}” scheduled`);
      if (ok) taskName = '';
    }
    if (ok) showForm = false;
  }

  function toggle(id: string, enabled: boolean) {
    const cmd = kind === 'alerts' ? 'toggle_alert' : kind === 'automations' ? 'toggle_automation' : 'toggle_scheduled_task';
    return run(() => call(cmd, { id, enabled }), enabled ? 'Enabled' : 'Disabled');
  }

  function remove(id: string, name: string) {
    if (!confirm(`Delete “${name}”?`)) return;
    const ruleKind = kind === 'alerts' ? 'alert' : kind === 'automations' ? 'automation' : 'task';
    return run(() => call('delete_rule', { ruleKind, id }), `Deleted “${name}”`);
  }

  function runNow(id: string, name: string) {
    return run(() => call('run_rule_now', { ruleKind: kind === 'automations' ? 'automation' : 'task', id }), `Running “${name}”… (result appears below and as a notification)`);
  }

  // --- presets
  const alertPresets: { name: string; condition: Condition }[] = [
    { name: 'GPU running hot', condition: { metric: 'gpu_temp', op: 'above', threshold: 85, sustain_secs: 60 } },
    { name: 'Disk almost full', condition: { metric: 'disk', op: 'above', threshold: 90, sustain_secs: 0 } },
    { name: 'Memory pressure', condition: { metric: 'memory', op: 'above', threshold: 92, sustain_secs: 120 } },
    { name: 'CPU pegged', condition: { metric: 'cpu', op: 'above', threshold: 95, sustain_secs: 300 } },
    { name: 'Ollama is down', condition: { metric: 'ollama_down', op: 'above', threshold: 0, sustain_secs: 60 } },
    { name: 'Memory service is down', condition: { metric: 'kb_core_down', op: 'above', threshold: 0, sustain_secs: 60 } }
  ];
  function preset(p: { name: string; condition: Condition }) {
    return run(() => call('create_alert', { input: { name: p.name, condition: p.condition, notify: true } }), `Alert “${p.name}” created`);
  }
  function briefing() {
    return run(
      () => call('create_scheduled_task', {
        input: {
          name: 'Morning briefing',
          schedule: '0 8 * * 1-5',
          action: { kind: 'ai_report', prompt: 'Morning briefing: health of this computer, anything that failed overnight, disk and GPU status, and what needs my attention today.', save_to_memory: true }
        }
      }),
      'Morning briefing scheduled for weekdays at 08:00'
    );
  }

  let rows = $derived(
    kind === 'alerts' ? data?.alerts ?? [] : kind === 'automations' ? data?.automations ?? [] : data?.tasks ?? []
  );
  let activityKinds = $derived(kind === 'alerts' ? ['alert_fired', 'alert_resolved'] : kind === 'automations' ? ['automation'] : ['schedule']);
  let activity = $derived((data?.activity ?? []).filter((a) => activityKinds.includes(a.kind)).slice(0, 15));
  let title = $derived(kind === 'alerts' ? 'Alerts' : kind === 'automations' ? 'Automation' : 'Scheduler');
  let blurb = $derived(
    kind === 'alerts'
      ? 'Watch a metric or service and get a desktop notification when a condition holds long enough. Alerts can also trigger automations.'
      : kind === 'automations'
        ? 'When something happens (an alert fires, a process starts or stops, a file changes, the computer goes idle), do something.'
        : 'Run a notification, a command or an AI report on a schedule. Uses cron syntax or presets.'
  );
  let canCreate = $derived(
    kind === 'alerts' ? alertName.trim().length > 0 : kind === 'automations' ? autoName.trim().length > 0 : taskName.trim().length > 0
  );
</script>

<div class="space-y-5">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <h3 class="text-xl font-bold text-cosmic-cyan">{title}</h3>
      <p class="text-sm text-gray-400 mt-1 max-w-2xl">{blurb}</p>
    </div>
    <button onclick={() => (showForm = !showForm)} class="px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg text-sm">{showForm ? 'Cancel' : '＋ New'}</button>
  </div>

  {#if kind === 'alerts' && !showForm}
    <div class="flex flex-wrap gap-2">
      <span class="text-xs text-gray-400 self-center">Quick add:</span>
      {#each alertPresets.filter((p) => !(data?.alerts ?? []).some((a) => a.alert.name === p.name)) as p}
        <button disabled={busy} onclick={() => preset(p)} class="text-xs px-2 py-1 rounded-full bg-white/5 hover:bg-white/10 disabled:opacity-40">{p.name}</button>
      {/each}
    </div>
  {:else if kind === 'tasks' && !showForm && !(data?.tasks ?? []).some((t) => t.task.name === 'Morning briefing')}
    <button disabled={busy} onclick={briefing} class="text-xs px-3 py-1.5 rounded-full bg-white/5 hover:bg-white/10 disabled:opacity-40">
      ☀ Quick add: weekday morning briefing (AI report at 08:00, saved to memory)
    </button>
  {/if}

  {#if showForm}
    <div class="glass-panel p-4 bg-white/5 space-y-3">
      {#if kind === 'alerts'}
        <input id="alert-name" bind:value={alertName} placeholder="Name, e.g. GPU running hot" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
        <ConditionEditor bind:condition={alertCond} idPrefix="alert" />
        <label class="flex items-center gap-2 text-sm"><input id="alert-notify" type="checkbox" bind:checked={alertNotify} /> Desktop notification when it fires</label>
      {:else if kind === 'automations'}
        <input id="auto-name" bind:value={autoName} placeholder="Name, e.g. Restart speech server when it dies" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
        <div>
          <label for="auto-trigger" class="block text-xs text-gray-400 mb-1">Trigger</label>
          <select id="auto-trigger" bind:value={triggerKind} class="w-full md:w-72 bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm">
            <option value="condition">A condition holds</option>
            <option value="alert">An alert fires</option>
            <option value="process_start">A process starts</option>
            <option value="process_stop">A process stops</option>
            <option value="file_change">A file changes</option>
            <option value="idle">The computer is idle</option>
          </select>
        </div>
        {#if triggerKind === 'condition'}
          <ConditionEditor bind:condition={trigCond} idPrefix="auto-cond" />
        {:else if triggerKind === 'alert'}
          <select id="auto-alert" bind:value={trigAlert} class="w-full bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm">
            <option value="">Choose an alert…</option>
            {#each data?.alerts ?? [] as a}<option value={a.alert.id}>{a.alert.name}: {a.description}</option>{/each}
          </select>
        {:else if triggerKind === 'process_start' || triggerKind === 'process_stop'}
          <input id="auto-proc" bind:value={trigProcess} placeholder="Process name, e.g. steam" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
        {:else if triggerKind === 'file_change'}
          <input id="auto-path" bind:value={trigPath} placeholder="/home/you/notes/todo.md" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
        {:else}
          <div class="flex gap-2 items-center text-sm">
            CPU below <input id="auto-idle-cpu" type="number" bind:value={idleCpu} class="w-20 bg-white/5 border border-white/10 rounded-lg px-2 py-1" />%
            for <input id="auto-idle-min" type="number" bind:value={idleMinutes} class="w-20 bg-white/5 border border-white/10 rounded-lg px-2 py-1" /> minutes
          </div>
        {/if}
        <ActionEditor bind:action={autoAction} idPrefix="auto-action" />
      {:else}
        <input id="task-name" bind:value={taskName} placeholder="Name, e.g. Nightly notes backup" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
        <div class="flex flex-wrap gap-2 items-end">
          <div>
            <label for="task-schedule" class="block text-xs text-gray-400 mb-1">Schedule (cron: minute hour day month weekday, or @every 30m)</label>
            <input id="task-schedule" bind:value={schedule} class="font-mono bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
          </div>
          <select id="task-preset" onchange={(e) => (schedule = e.currentTarget.value)} class="bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm">
            <option value="">Presets…</option>
            {#each schedules as p}<option value={p.v}>{p.l}</option>{/each}
          </select>
        </div>
        <ActionEditor bind:action={taskAction} idPrefix="task-action" />
      {/if}
      <button onclick={create} disabled={busy || !canCreate} class="px-4 py-2 bg-cosmic-blue hover:bg-cosmic-cyan text-white rounded-lg text-sm disabled:opacity-50">
        {busy ? 'Saving…' : 'Create'}
      </button>
    </div>
  {/if}

  <div class="space-y-2">
    {#if kind === 'alerts'}
      {#each data?.alerts ?? [] as a (a.alert.id)}
        <div class="glass-panel p-3 bg-white/5 flex flex-wrap items-center gap-3 {a.alert.state.firing ? 'border border-red-500/40' : ''}">
          <span class="w-2.5 h-2.5 rounded-full shrink-0 {!a.alert.enabled ? 'bg-gray-500' : a.alert.state.firing ? 'bg-red-400 animate-pulse' : 'bg-green-400'}"></span>
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium">{a.alert.name} {#if a.alert.state.firing}<span class="text-xs text-red-300 ml-1">firing since {ago(a.alert.state.since)}</span>{/if}</div>
            <div class="text-xs text-gray-400">{a.description}{#if a.value != null && a.unit} · now {a.value.toFixed(0)}{a.unit}{/if} · fired {a.alert.state.fire_count}× {#if a.alert.state.last_fired}(last {ago(a.alert.state.last_fired)}){/if}</div>
          </div>
          <label class="flex items-center gap-1 text-xs"><input type="checkbox" checked={a.alert.enabled} onchange={(e) => toggle(a.alert.id, e.currentTarget.checked)} /> on</label>
          <button onclick={() => remove(a.alert.id, a.alert.name)} class="text-xs text-red-400 hover:text-red-300 px-2">Delete</button>
        </div>
      {/each}
    {:else if kind === 'automations'}
      {#each data?.automations ?? [] as a (a.automation.id)}
        <div class="glass-panel p-3 bg-white/5 flex flex-wrap items-center gap-3">
          <span class="w-2.5 h-2.5 rounded-full shrink-0 {!a.automation.enabled ? 'bg-gray-500' : a.automation.run.last_ok === false ? 'bg-red-400' : 'bg-green-400'}"></span>
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium">{a.automation.name} {#if a.approved === true}<span class="text-xs text-green-300 ml-1" title="You approved this command for unattended runs">✓ approved</span>{/if}</div>
            <div class="text-xs text-gray-400">{a.trigger} → {a.action}</div>
            <div class="text-xs text-gray-500">ran {a.automation.run.run_count}× {#if a.automation.run.last_run}· last {ago(a.automation.run.last_run)}: {a.automation.run.last_result}{/if}</div>
          </div>
          <label class="flex items-center gap-1 text-xs"><input type="checkbox" checked={a.automation.enabled} onchange={(e) => toggle(a.automation.id, e.currentTarget.checked)} /> on</label>
          <button disabled={busy} onclick={() => runNow(a.automation.id, a.automation.name)} class="text-xs px-2 py-1 rounded bg-white/5 hover:bg-white/10">Run now</button>
          <button onclick={() => remove(a.automation.id, a.automation.name)} class="text-xs text-red-400 hover:text-red-300 px-2">Delete</button>
        </div>
      {/each}
    {:else}
      {#each data?.tasks ?? [] as t (t.task.id)}
        <div class="glass-panel p-3 bg-white/5 flex flex-wrap items-center gap-3">
          <span class="w-2.5 h-2.5 rounded-full shrink-0 {!t.task.enabled ? 'bg-gray-500' : t.task.run.last_ok === false ? 'bg-red-400' : 'bg-green-400'}"></span>
          <div class="min-w-0 flex-1">
            <div class="text-sm font-medium">{t.task.name} {#if t.approved === true}<span class="text-xs text-green-300 ml-1">✓ approved</span>{/if}</div>
            <div class="text-xs text-gray-400">{t.when} → {t.action}{#if t.task.enabled && t.task.next_run} · next {ago(t.task.next_run)}{/if}</div>
            <div class="text-xs text-gray-500">ran {t.task.run.run_count}× {#if t.task.run.last_run}· last {ago(t.task.run.last_run)}: {t.task.run.last_result}{/if}</div>
          </div>
          <label class="flex items-center gap-1 text-xs"><input type="checkbox" checked={t.task.enabled} onchange={(e) => toggle(t.task.id, e.currentTarget.checked)} /> on</label>
          <button disabled={busy} onclick={() => runNow(t.task.id, t.task.name)} class="text-xs px-2 py-1 rounded bg-white/5 hover:bg-white/10">Run now</button>
          <button onclick={() => remove(t.task.id, t.task.name)} class="text-xs text-red-400 hover:text-red-300 px-2">Delete</button>
        </div>
      {/each}
    {/if}
    {#if rows.length === 0}<p class="text-sm text-gray-400">Nothing here yet.</p>{/if}
  </div>

  {#if activity.length}
    <div class="glass-panel p-4 bg-white/5">
      <h4 class="font-bold text-sm mb-2">Recent activity</h4>
      <div class="space-y-1 text-sm">
        {#each activity as ev (ev.ts + ev.name + ev.kind)}
          <div class="flex gap-3">
            <span class="{ev.ok ? 'text-green-400' : ev.kind === 'alert_fired' ? 'text-red-400' : 'text-yellow-400'}">{ev.ok ? '✓' : '⚠'}</span>
            <span class="text-gray-400 w-20 shrink-0">{ago(ev.ts)}</span>
            <span class="font-medium shrink-0">{ev.name}</span>
            <span class="text-gray-400 truncate" title={ev.summary}>{ev.summary}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
