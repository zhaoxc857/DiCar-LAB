"""Pure helpers for the automatic parameter sweep workflow."""

from __future__ import annotations


def parse_candidates(text: str) -> list:
    """Parse a comma/space separated candidate list into sorted floats."""
    values = []
    for token in str(text or "").replace("，", ",").replace(" ", ",").split(","):
        token = token.strip()
        if not token:
            continue
        values.append(float(token))
    return sorted(values)


def pick_best(results: list) -> dict | None:
    """Return the result dict with the lowest score, None when empty."""
    scored = [r for r in results if isinstance(r, dict) and "score" in r]
    if not scored:
        return None
    return min(scored, key=lambda r: float(r["score"]))
