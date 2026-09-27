<script lang="ts">
  import { renderMarkdown } from '$lib/markdown';
  // Bundled at build time: the help text ships inside the app and works offline.
  import help from '$lib/help/HELP.md?raw';

  const html = renderMarkdown(help);
</script>

<div class="glass-panel p-8 overflow-auto h-full">
  <article class="help-md max-w-3xl mx-auto text-gray-200">
    <!-- Our own bundled file, still passed through the same DOMPurify pipeline as chat. -->
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    {@html html}
  </article>
</div>

<style>
  .help-md :global(h1) { font-size: 1.75rem; font-weight: 700; color: #00d4ff; margin: 0 0 1rem; }
  .help-md :global(h2) { font-size: 1.3rem; font-weight: 600; color: #7dd3fc; margin: 1.75rem 0 0.5rem; }
  .help-md :global(h3) { font-size: 1.05rem; font-weight: 600; margin: 1.25rem 0 0.4rem; }
  .help-md :global(p), .help-md :global(li) { line-height: 1.6; margin: 0.4rem 0; }
  .help-md :global(ul), .help-md :global(ol) { padding-left: 1.4rem; list-style: revert; }
  .help-md :global(code) { background: rgba(255, 255, 255, 0.08); padding: 0.1rem 0.35rem; border-radius: 4px; font-size: 0.9em; }
  .help-md :global(pre) { background: rgba(0, 0, 0, 0.35); padding: 0.8rem 1rem; border-radius: 8px; overflow-x: auto; margin: 0.6rem 0; }
  .help-md :global(pre code) { background: none; padding: 0; }
  .help-md :global(table) { border-collapse: collapse; margin: 0.6rem 0; font-size: 0.9rem; }
  .help-md :global(th), .help-md :global(td) { border: 1px solid rgba(255, 255, 255, 0.12); padding: 0.35rem 0.6rem; text-align: left; }
</style>
