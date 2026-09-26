// Build a styled HTML preview of the Medium article (docs/articles/medium-omnix-kb-core.md).
//   node docs/articles/build-preview.mjs   →   docs/articles/omnix-article-preview.html
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { marked } from 'marked';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..');
const md = readFileSync(join(here, 'medium-omnix-kb-core.md'), 'utf8').replace(/<!--[\s\S]*?-->\s*/, '');

let body = marked.parse(md);
body = body.replace(/^<h1>[\s\S]*?<\/h1>\s*<p><em>[\s\S]*?<\/em><\/p>\s*<hr>\s*/, '');
body = body
  .replaceAll('<table>', '<div class="scroll"><table>')
  .replaceAll('</table>', '</table></div>')
  .replaceAll('<pre>', '<div class="scroll"><pre>')
  .replaceAll('</pre>', '</pre></div>');

const slots = {
  '<h2>Part 1: An agent that can&#39;t go rogue</h2>':
    'Screenshot: the native approval dialog for an AI-proposed command (who proposed it, the exact command, the risk tier).',
  '<h2>How it comes together in a conversation</h2>':
    'Screenshot: a chat reply showing the “✓ memory_recall” step and a “🧠 Remembered: …” note.',
  '<h2>What makes OMNIX different</h2>':
    'Screenshot: Knowledge → Analytics (memories by category, engine health, recent activity).',
};
for (const [h, cap] of Object.entries(slots)) {
  if (!body.includes(h)) throw new Error(`heading not found: ${h}`);
  body = body.replace(
    h,
    `${h}\n<figure class="slot"><div class="slot-box" aria-hidden="true"><span>Image slot</span></div><figcaption>${cap}</figcaption></figure>`,
  );
}

const hero = readFileSync(join(root, 'uiexample.jpg')).toString('base64');
const words = body.replace(/<[^>]+>/g, ' ').split(/\s+/).filter(Boolean).length;
const mins = Math.round(words / 230);
const title = 'I Built an AI Assistant That Remembers Everything — and Asks Before It Touches Anything';
const sub =
  'Inside OMNIX and kb-core: a local-first desktop agent with a Rust security boundary and a memory engine that knows when it’s being contradicted.';
const tags = ['Artificial Intelligence', 'Rust', 'Local LLM', 'Cybersecurity', 'Software Engineering'];

const css = readFileSync(join(here, 'preview.css'), 'utf8');

const html = `<title>OMNIX Article Preview</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,500;12..96,700&family=Newsreader:ital,opsz,wght@0,6..72,400;0,6..72,600;1,6..72,400&family=JetBrains+Mono:wght@400;600&display=swap">
<style>
${css}
</style>

<div class="wrap">
  <div class="status"><span><b>Draft preview</b> · not published</span><span>${words.toLocaleString('en-US')} words · ${mins} min read</span></div>

  <details class="notes">
    <summary>Publishing notes for Medium</summary>
    <div class="inner">
      <div class="field"><span class="label">Title</span>
        <div class="row"><span class="val" id="t-title">${title}</span><button class="copy" type="button" data-copy="t-title">Copy</button></div></div>
      <div class="field"><span class="label">Subtitle</span>
        <div class="row"><span class="val" id="t-sub">${sub}</span><button class="copy" type="button" data-copy="t-sub">Copy</button></div></div>
      <div class="field"><span class="label">Tags (5)</span>
        <div class="row"><ul class="tags val" id="t-tags">${tags.map((t) => `<li>${t}</li>`).join('')}</ul><button class="copy" type="button" data-copy="t-tags">Copy</button></div></div>
      <p>Replace the three dashed image slots with screenshots before publishing. The hero image is the real OMNIX interface (uiexample.jpg). The source text lives in docs/articles/medium-omnix-kb-core.md.</p>
    </div>
  </details>

  <header class="hero">
    <p class="kicker">OMNIX · kb-core · local-first AI</p>
    <h1>${title}</h1>
    <p class="sub">${sub}</p>
    <div class="byline"><div class="avatar" aria-hidden="true">PM</div>
      <div><div>Paul Moore</div><div class="meta">Moore Core Technologies · ${mins} min read</div></div></div>
    <figure class="hero-img"><img alt="The OMNIX desktop interface with its holographic avatar, sidebar and chat input" src="data:image/jpeg;base64,${hero}">
      <figcaption>OMNIX: the holographic avatar shows mood, alerts and every tool call as it happens.</figcaption></figure>
  </header>

  <article>
${body}
  </article>
</div>
<script>
document.querySelectorAll('.copy').forEach(function (btn) {
  btn.addEventListener('click', function () {
    var el = document.getElementById(btn.dataset.copy);
    var text = el.tagName === 'UL' ? Array.from(el.children).map(function (li) { return li.textContent; }).join(', ') : el.textContent;
    function done() { btn.textContent = 'Copied'; setTimeout(function () { btn.textContent = 'Copy'; }, 1600); }
    function fallback() { var r = document.createRange(); r.selectNodeContents(el); var s = getSelection(); s.removeAllRanges(); s.addRange(r); btn.textContent = 'Selected'; }
    try { navigator.clipboard.writeText(text).then(done, fallback); } catch (e) { fallback(); }
  });
});
</script>
`;
writeFileSync(join(here, 'omnix-article-preview.html'), html);
console.log(`wrote omnix-article-preview.html (${words} words, ${mins} min)`);
