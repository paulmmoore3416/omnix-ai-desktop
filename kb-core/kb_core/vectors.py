"""Vector storage and exact nearest-neighbour search.

Vectors are L2-normalised float32, so cosine similarity is a dot product.
The whole index lives in memory: exact search over 100k 768-d vectors is a
few milliseconds with numpy and needs no approximate index to tune or
corrupt. Without numpy the same API runs in pure Python (slower, but
correct), so the service still works on a bare interpreter.
"""

from __future__ import annotations

import array
import math
import threading
from typing import Iterable, Sequence

try:  # pragma: no cover - exercised implicitly depending on the environment
    import numpy as np
except ImportError:  # pragma: no cover
    np = None  # type: ignore[assignment]

HAVE_NUMPY = np is not None


def normalise(v: Sequence[float]) -> array.array:
    n = math.sqrt(sum(x * x for x in v)) or 1.0
    return array.array("f", (x / n for x in v))


def to_blob(v: array.array) -> bytes:
    return v.tobytes()


def from_blob(b: bytes) -> array.array:
    a = array.array("f")
    a.frombytes(b)
    return a


def dot(a: Sequence[float], b: Sequence[float]) -> float:
    if HAVE_NUMPY:
        return float(np.dot(np.asarray(a, dtype=np.float32), np.asarray(b, dtype=np.float32)))
    return sum(x * y for x, y in zip(a, b))


class VectorIndex:
    """Thread-safe id → vector map with exact top-k search.

    With numpy, vectors live in one preallocated matrix that doubles when
    full (amortised O(1) appends, no rebuild after every write); removal
    swaps the last row into the hole (O(1)). ``dtype="float16"`` halves the
    memory (≈1.5 KB per 768-d vector); scoring is done in float32 chunks so
    precision loss stays negligible for ranking. Without numpy, a plain list
    of arrays is scanned in pure Python.
    """

    CHUNK = 65_536

    def __init__(self, dtype: str = "float32") -> None:
        self._lock = threading.RLock()
        self._ids: list[str] = []
        self._pos: dict[str, int] = {}
        self._vecs: list[array.array] = []  # pure-Python mode only
        self._mat = None  # numpy mode: capacity × dim
        self._dtype = dtype if dtype in {"float32", "float16"} else "float32"
        self.dim = 0

    def __len__(self) -> int:
        return len(self._ids)

    def __contains__(self, item_id: str) -> bool:
        return item_id in self._pos

    @property
    def memory_bytes(self) -> int:
        if HAVE_NUMPY and self._mat is not None:
            return int(self._mat.nbytes)
        return len(self._vecs) * self.dim * 4

    def load(self, rows: Iterable[tuple[str, bytes]]) -> None:
        with self._lock:
            self._ids, self._pos, self._vecs, self._mat = [], {}, [], None
            self.dim = 0
            for item_id, blob in rows:
                self._put(item_id, from_blob(blob))

    def _grow(self, need: int) -> None:
        cap = 0 if self._mat is None else self._mat.shape[0]
        if need <= cap:
            return
        new_cap = max(1024, cap * 2, need)
        m = np.zeros((new_cap, self.dim), dtype=self._dtype)
        if self._mat is not None:
            m[: len(self._ids)] = self._mat[: len(self._ids)]
        self._mat = m

    def _put(self, item_id: str, vec: array.array) -> None:
        if self.dim and len(vec) != self.dim:
            raise ValueError(f"vector dimension {len(vec)} does not match index dimension {self.dim}")
        self.dim = len(vec)
        row = self._pos.get(item_id)
        if HAVE_NUMPY:
            if row is None:
                row = len(self._ids)
                self._grow(row + 1)
                self._pos[item_id] = row
                self._ids.append(item_id)
            self._mat[row] = np.frombuffer(vec.tobytes(), dtype=np.float32)
        elif row is None:
            self._pos[item_id] = len(self._ids)
            self._ids.append(item_id)
            self._vecs.append(vec)
        else:
            self._vecs[row] = vec

    def add(self, item_id: str, vec: array.array) -> None:
        with self._lock:
            self._put(item_id, vec)

    def remove(self, item_ids: Iterable[str]) -> None:
        with self._lock:
            for item_id in item_ids:
                row = self._pos.pop(item_id, None)
                if row is None:
                    continue
                last = len(self._ids) - 1
                if row != last:
                    moved = self._ids[last]
                    self._ids[row] = moved
                    self._pos[moved] = row
                    if HAVE_NUMPY:
                        self._mat[row] = self._mat[last]
                    else:
                        self._vecs[row] = self._vecs[last]
                self._ids.pop()
                if not HAVE_NUMPY:
                    self._vecs.pop()
            if not self._ids:
                self._mat, self._vecs, self.dim = None, [], 0

    def clear(self) -> None:
        self.load([])

    def get(self, item_id: str) -> array.array | None:
        with self._lock:
            p = self._pos.get(item_id)
            if p is None:
                return None
            if HAVE_NUMPY:
                return array.array("f", self._mat[p].astype(np.float32).tobytes())
            return self._vecs[p]

    def search(
        self, query: Sequence[float], k: int, allow: set[str] | None = None
    ) -> list[tuple[str, float]]:
        """Top-``k`` (id, cosine) pairs, best first. ``allow`` restricts ids."""
        return self.scan(query, k, allow)[0]

    def scan(
        self, query: Sequence[float], k: int, allow: set[str] | None = None
    ) -> tuple[list[tuple[str, float]], float, int]:
        """Like :meth:`search`, plus the mean similarity over the *whole*
        index and its size (the query's background level, used to calibrate
        relevance)."""
        with self._lock:
            n = len(self._ids)
            if not n or k <= 0 or len(query) != self.dim:
                return [], 0.0, 0
            if HAVE_NUMPY:
                q = np.asarray(query, dtype=np.float32)
                scores = np.empty(n, dtype=np.float32)
                for i in range(0, n, self.CHUNK):
                    block = self._mat[i : min(n, i + self.CHUNK)]
                    scores[i : i + len(block)] = block.astype(np.float32, copy=False) @ q
                mean = float(scores.mean())
                ids = self._ids
                if allow is not None:
                    mask = np.fromiter((i in allow for i in ids), dtype=bool, count=n)
                    scores = np.where(mask, scores, -np.inf)
                k = min(k, n)
                top = np.argpartition(-scores, k - 1)[:k]
                top = top[np.argsort(-scores[top])]
                return [(ids[i], float(scores[i])) for i in top if np.isfinite(scores[i])], mean, n
            all_pairs = [(i, sum(x * y for x, y in zip(v, query))) for i, v in zip(self._ids, self._vecs)]
            mean = sum(s for _, s in all_pairs) / n
            pairs = [p for p in all_pairs if allow is None or p[0] in allow]
            pairs.sort(key=lambda p: p[1], reverse=True)
            return pairs[:k], mean, n
