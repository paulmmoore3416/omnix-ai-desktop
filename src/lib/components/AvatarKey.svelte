<script lang="ts">
  import { CONDITIONS, MOODS, SIGNALS, rgba, type Condition, type Mood } from '$lib/avatar';

  /** Colour key for the OMNIX avatar. Highlights whatever is showing right now. */
  let { mood = 'idle', condition = 'nominal' }: { mood?: Mood; condition?: Condition } = $props();

  const sections = [
    { title: 'Mood', hint: 'Core and inner rings: what OMNIX is doing', rows: Object.entries(MOODS) },
    { title: 'Condition', hint: 'Outer halo and warning banner: a standing problem', rows: Object.entries(CONDITIONS) },
    { title: 'Signal', hint: 'A ripple from the core: a one-off event', rows: Object.entries(SIGNALS) }
  ];

  const isActive = (title: string, key: string) =>
    (title === 'Mood' && key === mood) || (title === 'Condition' && key === condition);
</script>

<div class="key" role="region" aria-label="Avatar colour key">
  {#each sections as s}
    <section>
      <h3>{s.title} <span>{s.hint}</span></h3>
      <ul>
        {#each s.rows as [key, sw]}
          <li class:active={isActive(s.title, key)} title={sw.effect}>
            <i style="background: {sw.color}; box-shadow: 0 0 8px {rgba(sw.color, 0.8)};"></i>
            <b style="color: {sw.color};">{sw.label}</b>
            <em>{sw.meaning}</em>
            <small>{sw.effect}</small>
          </li>
        {/each}
      </ul>
    </section>
  {/each}
</div>

<style>
  .key {
    width: min(640px, 100%);
    margin-top: 12px;
    padding: 14px 16px;
    border-radius: 16px;
    background: rgba(8, 14, 30, 0.85);
    border: 1px solid rgba(120, 200, 255, 0.18);
    font-family: ui-monospace, 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 11px;
    color: #c9d6ea;
  }
  section + section { margin-top: 12px; }
  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    letter-spacing: 2px;
    text-transform: uppercase;
    color: #7fdcff;
  }
  h3 span {
    margin-left: 6px;
    letter-spacing: 0;
    text-transform: none;
    color: #6f7f99;
    font-weight: 400;
  }
  ul {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 2px 14px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: grid;
    grid-template-columns: 12px auto 1fr;
    grid-template-rows: auto auto;
    column-gap: 8px;
    align-items: center;
    padding: 4px 6px;
    border-radius: 8px;
    border: 1px solid transparent;
  }
  li.active {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.15);
  }
  i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    grid-row: span 2;
  }
  b { font-weight: 700; white-space: nowrap; }
  em { font-style: normal; color: #a8b6cc; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small { grid-column: 2 / 4; color: #6f7f99; font-size: 10px; }
</style>
