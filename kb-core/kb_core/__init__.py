"""kb-core: OMNIX's local long-term memory engine.

A self-contained service that gives OMNIX durable memory and a searchable
knowledge base, running entirely on the user's machine:

* hybrid retrieval (BM25 keyword + dense vectors) with a calibrated 0-1
  relevance, personalised multi-query matching, and maximal-marginal-
  relevance diversification;
* memory dynamics: recency, importance, reinforcement and recall frequency
  shape ranking, near-duplicates are merged instead of piling up, and
  contradicted facts are superseded rather than silently coexisting;
* automatic fact capture from conversations with a local LLM;
* live folder sync: watched note folders are re-indexed incrementally;
* resilience: saves never fail because the embedding model is down; missing
  vectors are backfilled, and changing the embedding model re-embeds in the
  background while keyword search keeps working.

See ``README.md`` and ``docs/kb-core-contract.md`` in the OMNIX repository.
"""

__version__ = "2.1.0"
