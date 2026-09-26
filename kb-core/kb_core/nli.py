"""Optional natural-language-inference (NLI) judge for memory consolidation.

A small cross-encoder trained on entailment/contradiction/neutral (by
default ``cross-encoder/nli-deberta-v3-xsmall``, ONNX, int8) scores a pair
of statements in ~15 ms on a CPU. kb-core uses it as a second, independent
opinion next to the local LLM when deciding whether a new memory duplicates
or replaces an older one: an 8B chat model at temperature 0 tends to call
anything that shares entities a duplicate, while an NLI model is trained for
exactly this three-way decision. It has its own failure mode (it can call
two unrelated facts about the same person a contradiction), which is why
candidates only reach it above the related-cosine threshold and why, when
both judges are available, a merge or supersession needs both to agree.

Needs ``onnxruntime`` and ``tokenizers`` (the installer adds them to the
venv and downloads the model into ``KB_CORE_NLI_MODEL``). Without them the
judge is simply off; nothing else depends on it.
"""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any

from .llm import ModelUnavailable

log = logging.getLogger("kb-core.nli")

MAX_TOKENS = 256
# Model file names tried in order: the quantised CPU build first.
MODEL_FILES = ("model_quint8_avx2.onnx", "model.onnx")

# Decision thresholds. A duplicate needs the new statement to entail the old
# one (the old adds nothing); a replacement needs contradiction in both
# directions (contradiction is symmetric; a one-way score is noise).
# 0.9, not 0.8: nli-deberta-v3-xsmall gives 0.85 entailment to "the server
# has an RX 580 and a GTX 1060" → "the server has two NVIDIA GPUs"; true
# paraphrases score 0.93+.
DUPLICATE_ENTAILMENT = 0.9
DUPLICATE_MAX_CONTRADICTION = 0.2
OBSOLETE_CONTRADICTION = 0.8


class NliJudge:
    """ONNX NLI cross-encoder. Thread-safe (onnxruntime sessions are)."""

    def __init__(self, model_dir: Path) -> None:
        try:
            import numpy as np
            import onnxruntime as ort
            from tokenizers import Tokenizer
        except ImportError as e:
            raise ModelUnavailable(f"NLI judge needs numpy, onnxruntime and tokenizers ({e.name} missing)") from e
        self._np = np
        model_dir = Path(model_dir).expanduser()
        model = next((model_dir / f for f in MODEL_FILES if (model_dir / f).is_file()), None)
        if model is None or not (model_dir / "tokenizer.json").is_file():
            raise ModelUnavailable(f"no NLI model in {model_dir} (expected {MODEL_FILES[0]} and tokenizer.json)")
        try:
            cfg = json.loads((model_dir / "config.json").read_text(encoding="utf-8"))
            id2label = {int(k): v.lower() for k, v in cfg["id2label"].items()}
        except (OSError, ValueError, KeyError) as e:
            raise ModelUnavailable(f"{model_dir}/config.json has no id2label: {e}") from e
        if set(id2label.values()) != {"contradiction", "entailment", "neutral"}:
            raise ModelUnavailable(f"{model_dir} is not a three-way NLI model: {sorted(id2label.values())}")
        self._labels = [id2label[i] for i in range(len(id2label))]
        opts = ort.SessionOptions()
        opts.intra_op_num_threads = 2  # a background judge; leave the CPU to the user
        opts.log_severity_level = 3
        self._session = ort.InferenceSession(str(model), opts, providers=["CPUExecutionProvider"])
        self._inputs = {i.name for i in self._session.get_inputs()}
        self._tok = Tokenizer.from_file(str(model_dir / "tokenizer.json"))
        self._tok.enable_truncation(MAX_TOKENS)
        pad = self._tok.token_to_id("[PAD]")
        self._tok.enable_padding(pad_id=pad if pad is not None else 0, pad_token="[PAD]")
        self.model = model_dir.name
        log.info("NLI judge loaded: %s (%s)", self.model, model.name)

    def probs(self, pairs: list[tuple[str, str]]) -> list[dict[str, float]]:
        """``[(premise, hypothesis), …]`` → ``[{"entailment", "contradiction", "neutral"}, …]``."""
        if not pairs:
            return []
        np = self._np
        enc = self._tok.encode_batch([(p[:4000], h[:4000]) for p, h in pairs])
        feed: dict[str, Any] = {
            "input_ids": np.array([e.ids for e in enc], dtype=np.int64),
            "attention_mask": np.array([e.attention_mask for e in enc], dtype=np.int64),
        }
        if "token_type_ids" in self._inputs:
            feed["token_type_ids"] = np.array([e.type_ids for e in enc], dtype=np.int64)
        try:
            logits = self._session.run(None, feed)[0]
        except Exception as e:  # noqa: BLE001 - onnxruntime raises its own types
            raise ModelUnavailable(f"NLI inference failed: {e}") from e
        z = np.exp(logits - logits.max(axis=1, keepdims=True))
        p = z / z.sum(axis=1, keepdims=True)
        return [dict(zip(self._labels, map(float, row))) for row in p]

    def rule(self, new: str, olds: list[str]) -> tuple[set[int], set[int], dict[int, float]]:
        """Judge ``new`` against each of ``olds`` (1-based numbering, like the
        LLM prompt). Returns ``(duplicates, obsolete, confidence)``."""
        pairs: list[tuple[str, str]] = []
        for o in olds:
            pairs += [(new, o), (o, new)]
        p = self.probs(pairs)
        dups: set[int] = set()
        obsolete: set[int] = set()
        conf: dict[int, float] = {}
        for n in range(1, len(olds) + 1):
            fwd, rev = p[2 * n - 2], p[2 * n - 1]
            contradiction = min(fwd["contradiction"], rev["contradiction"])
            if contradiction >= OBSOLETE_CONTRADICTION:
                obsolete.add(n)
                conf[n] = contradiction
            elif (fwd["entailment"] >= DUPLICATE_ENTAILMENT
                  and max(fwd["contradiction"], rev["contradiction"]) < DUPLICATE_MAX_CONTRADICTION):
                dups.add(n)
                conf[n] = fwd["entailment"]
        return dups, obsolete, conf
