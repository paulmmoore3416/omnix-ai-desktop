<script lang="ts">
  import type { OpsAction } from '$lib/types';

  let { action = $bindable(), idPrefix }: { action: OpsAction; idPrefix: string } = $props();

  function setKind(kind: OpsAction['kind']) {
    action =
      kind === 'notify'
        ? { kind, title: '', message: '' }
        : kind === 'command'
          ? { kind, command: '', cwd: null }
          : { kind, prompt: 'Summarise the health of this computer and anything that needs my attention.', save_to_memory: false };
  }
</script>

<div class="space-y-2">
  <div>
    <label for="{idPrefix}-kind" class="block text-xs text-gray-400 mb-1">Then</label>
    <select id="{idPrefix}-kind" value={action.kind} onchange={(e) => setKind(e.currentTarget.value as OpsAction['kind'])}
      class="w-full md:w-72 bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-sm">
      <option value="notify">Show a notification</option>
      <option value="command">Run a command</option>
      <option value="ai_report">Write an AI report</option>
    </select>
  </div>
  {#if action.kind === 'notify'}
    <div class="grid grid-cols-1 md:grid-cols-3 gap-2">
      <input id="{idPrefix}-title" bind:value={action.title} placeholder="Title" class="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
      <input id="{idPrefix}-message" bind:value={action.message} placeholder="Message" class="md:col-span-2 bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
    </div>
  {:else if action.kind === 'command'}
    <input id="{idPrefix}-command" bind:value={action.command} placeholder="e.g. rsync -a ~/notes /mnt/backup/notes" class="w-full font-mono bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
    <input id="{idPrefix}-cwd" bind:value={action.cwd} placeholder="Working directory (default: home)" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm" />
    <p class="text-xs text-gray-400">
      Read-only commands just run. Anything that changes your system asks for your approval <em>once</em>, now; OMNIX then runs exactly that
      command unattended. Changing it later needs approval again. Admin (sudo) commands and blocked commands can't be scheduled.
    </p>
  {:else}
    <textarea id="{idPrefix}-prompt" bind:value={action.prompt} rows="2" class="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm"></textarea>
    <label class="flex items-center gap-2 text-sm">
      <input id="{idPrefix}-save" type="checkbox" bind:checked={action.save_to_memory} /> Save each report to long-term memory
    </label>
    <p class="text-xs text-gray-400">The local model gets a measured snapshot of this computer (metrics, GPUs, services, containers, models, alerts) and writes a short report. No tools, no commands.</p>
  {/if}
</div>
