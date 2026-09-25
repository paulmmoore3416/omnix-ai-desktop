import { describe, expect, it } from 'vitest';
import { renderMarkdown } from './markdown';

describe('renderMarkdown (sanitized)', () => {
  it('renders basic markdown', () => {
    const html = renderMarkdown('**bold** and `code`\n\n```\nls -la\n```');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).toContain('<code>code</code>');
    expect(html).toContain('<pre>');
  });

  it('strips scripts, iframes and event handlers', () => {
    const html = renderMarkdown(
      '<script>alert(1)</script><iframe src="https://evil"></iframe><div onclick="x()" onmouseover="y()">hi</div><svg onload="z()"></svg>'
    );
    expect(html).not.toMatch(/<script|<iframe|<svg/i);
    expect(html).not.toMatch(/onclick|onmouseover|onload/i);
    expect(html).toContain('hi');
  });

  it('removes images, forms and styles', () => {
    const html = renderMarkdown(
      '![x](https://tracker/p.png)<form action="https://evil"><input></form><style>body{}</style><p style="color:red">t</p>'
    );
    expect(html).not.toMatch(/<img|<form|<input|<style|style=/i);
  });

  it('neutralises links so they cannot navigate the app', () => {
    const html = renderMarkdown('[click](https://evil.example) [js](javascript:alert(1))');
    expect(html).not.toMatch(/href=/i);
    expect(html).toContain('title="https://evil.example"');
    expect(html).not.toMatch(/javascript:alert/);
  });

  it('escapes raw html entities inside code blocks', () => {
    const html = renderMarkdown('```\n<script>alert(1)</script>\n```');
    expect(html).not.toContain('<script>');
    expect(html).toContain('&lt;script&gt;');
  });
});
