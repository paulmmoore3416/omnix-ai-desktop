"""Structure-aware document chunking.

Each chunk carries a *context* breadcrumb (``notes.md › Backups › ZFS``) that
is embedded and indexed with the text, so a hit is self-explanatory to the
model without the rest of the document. Markdown headings define sections;
fenced code blocks are never split at a heading and are kept whole where
they fit; long sections are split at paragraph, line, then sentence
boundaries with a small overlap so facts spanning a boundary stay findable.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

TARGET_CHARS = 1400
OVERLAP_CHARS = 200
MIN_MERGE_CHARS = 350

HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*#*\s*$")
FENCE_RE = re.compile(r"^\s*(```|~~~)")
SENTENCE_RE = re.compile(r"(?<=[.!?])\s+")
SEP = " › "


@dataclass(frozen=True)
class Chunk:
    context: str
    text: str

    def embed_text(self) -> str:
        return f"{self.context}\n\n{self.text}" if self.context else self.text


def _sections(content: str) -> list[tuple[list[str], str]]:
    sections: list[tuple[list[str], str]] = []
    trail: list[str] = []
    buf: list[str] = []
    fence: str | None = None
    for line in content.splitlines():
        m_fence = FENCE_RE.match(line)
        if m_fence:
            marker = m_fence.group(1)
            fence = None if fence == marker else (fence or marker)
        m = None if fence else HEADING_RE.match(line)
        if m:
            if "\n".join(buf).strip():
                sections.append((list(trail), "\n".join(buf).strip()))
            buf = []
            level = len(m.group(1))
            trail = trail[: level - 1] + [""] * max(0, level - 1 - len(trail)) + [m.group(2).strip()]
        else:
            buf.append(line)
    if "\n".join(buf).strip():
        sections.append((list(trail), "\n".join(buf).strip()))
    return sections


def _units(text: str, limit: int) -> list[str]:
    """Break text into pieces no longer than ``limit``, preferring big units."""
    out: list[str] = []
    for para in re.split(r"\n\s*\n", text):
        if len(para) <= limit:
            out.append(para)
            continue
        for line in para.split("\n"):
            if len(line) <= limit:
                out.append(line)
                continue
            for sent in SENTENCE_RE.split(line):
                while len(sent) > limit:
                    out.append(sent[:limit])
                    sent = sent[limit:]
                if sent:
                    out.append(sent)
    return out


def split_text(text: str, limit: int = TARGET_CHARS, overlap: int = OVERLAP_CHARS) -> list[str]:
    if len(text) <= limit:
        return [text]
    chunks: list[str] = []
    buf = ""
    for unit in _units(text, limit):
        sep = "\n\n" if buf else ""
        if buf and len(buf) + len(sep) + len(unit) > limit:
            chunks.append(buf)
            # Carry the tail of the previous chunk forward, cut at a word.
            tail = buf[-overlap:] if overlap else ""
            if " " in tail:
                tail = tail[tail.index(" ") + 1 :]
            buf = f"…{tail}\n\n{unit}" if tail and len(tail) + len(unit) + 3 <= limit else unit
        else:
            buf += sep + unit
    if buf:
        chunks.append(buf)
    return chunks


def _rel(ctx: str, base: str) -> str:
    return ctx[len(base) :].removeprefix(SEP)


def chunk_document(name: str, content: str, limit: int = TARGET_CHARS) -> list[Chunk]:
    """Chunk ``content``; ``name`` heads every breadcrumb."""
    chunks: list[Chunk] = []
    pending: tuple[str, str] | None = None
    for trail, body in _sections(content.replace("\r\n", "\n")):
        context = SEP.join([name] + [t for t in trail if t])
        if pending:
            p_ctx, p_body = pending
            ps, cs = p_ctx.split(SEP), context.split(SEP)
            common: list[str] = []
            for a, b in zip(ps, cs):
                if a != b:
                    break
                common.append(a)
            # Fold a small child or sibling section into the pending chunk
            # (re-rooted at the shared parent) so short FAQ-style entries
            # don't become context-free fragments.
            near = common == ps or (len(common) >= 2 and len(common) >= len(ps) - 1)
            if near and len(p_body) + len(body) < limit and len(body) < MIN_MERGE_CHARS:
                base = SEP.join(common)
                if base != p_ctx:
                    p_body = f"{_rel(p_ctx, base)}:\n{p_body}"
                sub = _rel(context, base)
                pending = (base, f"{p_body}\n\n{sub + ':' + chr(10) if sub else ''}{body}")
                continue
            chunks.extend(Chunk(p_ctx, t) for t in split_text(p_body, limit))
        pending = (context, body)
    if pending:
        chunks.extend(Chunk(pending[0], t) for t in split_text(pending[1], limit))
    return chunks


# ------------------------------------------------------------------ code

CODE_DEF_RE = re.compile(
    r"^(?:(?:pub(?:\([^)]*\))?|export|default|async|public|private|protected|static|final|abstract|override|unsafe)\s+)*"
    r"(?:def|class|fn|impl|struct|enum|trait|mod|func|function|interface|type|module|object|record)\s+([A-Za-z_][\w:<>]*)"
)
SHELL_FN_RE = re.compile(r"^(?:function\s+)?([A-Za-z_][\w-]*)\s*\(\)\s*\{?")
SECTION_RE = re.compile(r"^\[+\s*([^\]]+?)\s*\]+\s*$")  # TOML / INI
YAML_KEY_RE = re.compile(r"^([A-Za-z_][\w.-]*):(?:\s|$)")
CONFIG_EXT = {".toml", ".ini", ".cfg", ".conf", ".yaml", ".yml", ".properties", ".env.example"}


def _boundary(line: str, ext: str) -> str | None:
    """Symbol name when ``line`` starts a new top-level unit."""
    if ext in {".yaml", ".yml"}:
        m = YAML_KEY_RE.match(line)
        return m.group(1) if m else None
    if ext in CONFIG_EXT:
        m = SECTION_RE.match(line.strip())
        return m.group(1) if m else None
    if ext in {".sh", ".bash", ".zsh"}:
        m = SHELL_FN_RE.match(line)
        return m.group(1) if m else None
    if line[:1].isspace():
        # Methods one indent level deep still count (Python classes, impl blocks).
        stripped = line.lstrip()
        if len(line) - len(stripped) > 4:
            return None
        line = stripped
    m = CODE_DEF_RE.match(line)
    return m.group(1) if m else None


def chunk_code(name: str, content: str, limit: int = TARGET_CHARS) -> list[Chunk]:
    """Chunk source or config text at definitions/sections; each chunk's
    breadcrumb names the file and the symbol (``setup.py › install``)."""
    ext = "." + name.rsplit(".", 1)[-1].lower() if "." in name else ""
    segments: list[tuple[str, list[str]]] = [("", [])]
    for line in content.replace("\r\n", "\n").split("\n"):
        sym = _boundary(line, ext)
        if sym and any(x.strip() for x in segments[-1][1]):
            segments.append((sym, [line]))
        else:
            if sym and not segments[-1][0]:
                segments[-1] = (sym, segments[-1][1])
            segments[-1][1].append(line)
    # Merge small neighbours so a file of one-liners doesn't become 200 chunks.
    merged: list[tuple[str, str]] = []
    for sym, lines in segments:
        body = "\n".join(lines).strip("\n")
        if not body.strip():
            continue
        if merged and len(merged[-1][1]) + len(body) < limit // 2:
            psym, pbody = merged[-1]
            label = psym if not sym or psym == sym else (f"{psym}, {sym}" if psym else sym)
            merged[-1] = (label[:120], f"{pbody}\n\n{body}")
        else:
            merged.append((sym, body))
    chunks: list[Chunk] = []
    for sym, body in merged:
        ctx = f"{name}{SEP}{sym}" if sym else name
        chunks.extend(Chunk(ctx, t) for t in split_text(body, limit, overlap=120))
    return chunks
