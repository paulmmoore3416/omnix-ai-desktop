"""Ollama client: embeddings and structured (JSON-schema) chat."""

from __future__ import annotations

import array
import json
import urllib.error
import urllib.request
from typing import Any, Protocol

from .vectors import normalise

EMBED_BATCH = 32


class ModelUnavailable(RuntimeError):
    """The embedding or chat model could not be reached."""


class Embedder(Protocol):
    model: str

    def documents(self, texts: list[str]) -> list[array.array]: ...

    def query(self, text: str) -> array.array: ...

    def queries(self, texts: list[str]) -> list[array.array]: ...


def _post(url: str, body: dict[str, Any], timeout: float) -> dict[str, Any]:
    req = urllib.request.Request(
        url, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        detail = e.read(300).decode("utf-8", "replace")
        raise ModelUnavailable(f"{url} returned {e.code}: {detail}") from e
    except (urllib.error.URLError, TimeoutError, OSError, ValueError) as e:
        raise ModelUnavailable(f"{url}: {e}") from e


class OllamaEmbedder:
    """``/api/embed`` client returning L2-normalised float32 vectors."""

    def __init__(self, base: str, model: str) -> None:
        self.base = base.rstrip("/")
        self.model = model
        # nomic-embed-text (and similar retrieval models) are trained with
        # asymmetric task prefixes; using them measurably improves recall.
        m = model.lower()
        if "nomic" in m:
            self._doc, self._query = "search_document: ", "search_query: "
        elif "mxbai" in m:
            self._doc, self._query = "", "Represent this sentence for searching relevant passages: "
        else:
            self._doc = self._query = ""

    def _embed(self, texts: list[str]) -> list[array.array]:
        out: list[array.array] = []
        for i in range(0, len(texts), EMBED_BATCH):
            data = _post(
                f"{self.base}/api/embed",
                {"model": self.model, "input": texts[i : i + EMBED_BATCH], "truncate": True},
                timeout=300,
            )
            try:
                out.extend(normalise(v) for v in data["embeddings"])
            except (KeyError, TypeError) as e:
                raise ModelUnavailable(f"unexpected embedding response from {self.model}") from e
        return out

    def documents(self, texts: list[str]) -> list[array.array]:
        return self._embed([self._doc + t for t in texts])

    def query(self, text: str) -> array.array:
        return self._embed([self._query + text])[0]

    def queries(self, texts: list[str]) -> list[array.array]:
        return self._embed([self._query + t for t in texts])


class OllamaChat:
    """Minimal non-streaming chat with Ollama's structured-output support."""

    def __init__(self, base: str, model: str) -> None:
        self.base = base.rstrip("/")
        self.model = model

    def json(self, system: str, user: str, schema: dict[str, Any], timeout: float = 180) -> Any:
        data = _post(
            f"{self.base}/api/chat",
            {
                "model": self.model,
                "stream": False,
                # Reasoning models would otherwise spend seconds "thinking"
                # about a mechanical extraction task.
                "think": False,
                "format": schema,
                "options": {"temperature": 0},
                "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
            },
            timeout=timeout,
        )
        try:
            return json.loads(data["message"]["content"])
        except (KeyError, TypeError, ValueError) as e:
            raise ModelUnavailable(f"{self.model} did not return valid JSON") from e
