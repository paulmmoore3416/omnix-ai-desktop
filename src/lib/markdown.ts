/**
 * Markdown → sanitized HTML for chat messages.
 *
 * Model output is untrusted (it can be steered by prompt injection), so the
 * HTML produced by `marked` is always passed through DOMPurify before it is
 * given to `{@html}`:
 *  - scripts, iframes, objects, embeds, forms, styles, images and media are
 *    removed (images would also be blocked by the CSP; removing them avoids
 *    broken-image noise and tracking attempts);
 *  - every `on*` event-handler attribute and inline `style` is removed;
 *  - links are neutralised: the `href` is moved to `title` so clicking can
 *    never navigate the app window to a remote origin.
 */
import DOMPurify from 'dompurify';
import { marked } from 'marked';

const FORBID_TAGS = [
  'script', 'iframe', 'object', 'embed', 'form', 'input', 'button', 'textarea', 'select',
  'style', 'link', 'meta', 'base', 'img', 'video', 'audio', 'source', 'svg', 'math', 'frame',
  'frameset'
];

let hooksInstalled = false;

function installHooks() {
  if (hooksInstalled) return;
  DOMPurify.addHook('uponSanitizeAttribute', (_node, data) => {
    // Belt and braces: DOMPurify already drops handlers, but never keep one.
    if (data.attrName.toLowerCase().startsWith('on')) data.keepAttr = false;
  });
  DOMPurify.addHook('afterSanitizeAttributes', (node) => {
    if (node.tagName === 'A') {
      const href = node.getAttribute('href');
      node.removeAttribute('href');
      node.removeAttribute('target');
      if (href) node.setAttribute('title', href);
      node.setAttribute('class', 'md-link');
    }
  });
  hooksInstalled = true;
}

/** Render untrusted Markdown to safe HTML. */
export function renderMarkdown(source: string): string {
  installHooks();
  const html = marked.parse(source, { async: false, gfm: true, breaks: true }) as string;
  return DOMPurify.sanitize(html, {
    FORBID_TAGS,
    FORBID_ATTR: ['style', 'srcset', 'formaction', 'xlink:href'],
    ALLOW_DATA_ATTR: false
  });
}
