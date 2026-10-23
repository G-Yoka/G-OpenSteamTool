"""Signature synthesis: turn a hook RVA into a byte-pattern string.

A signature is the byte prefix starting at the function entry point. It must be
long enough to be unique inside the target image and no longer than
:data:`SIG_CAP` bytes; longer payloads only slow the runtime scan down.

The trimming rule is the interesting part. We start from ``SIG_CAP`` bytes and
shorten to the *shortest* prefix whose occurrence count in the scan space is
exactly one. If even ``SIG_CAP`` bytes are ambiguous the hook is reported as
unresolved rather than published with a pattern that could match the wrong
function.
"""

from __future__ import annotations

SIG_CAP = 80
SIG_MIN = 8
# Published signatures never go below this length even when fewer bytes would
# already be unique: extra bytes cost nothing at scan time and a very short
# pattern is fragile against a hotfix that only shifts a few instructions.
SIG_FLOOR = 24


class SigError(RuntimeError):
    pass


def hex_bytes(raw: bytes) -> str:
    """``b'\\x48\\x8b'`` -> ``'48 8B'`` (upper-case, space separated)."""
    return " ".join(f"{b:02X}" for b in raw)


def hex_to_bytes(text: str) -> bytes:
    return bytes(int(part, 16) for part in text.split())


def occurrence_count(space: bytes, needle: bytes) -> int:
    count = 0
    start = 0
    while True:
        idx = space.find(needle, start)
        if idx < 0:
            return count
        count += 1
        start = idx + 1


def shortest_unique_prefix(space: bytes, needle: bytes) -> bytes:
    """Shortest prefix of ``needle`` occurring exactly once in ``space``."""
    for size in range(SIG_MIN, len(needle) + 1):
        prefix = needle[:size]
        if occurrence_count(space, prefix) == 1:
            return prefix
    raise SigError(
        f"no unique prefix within {len(needle)} bytes "
        f"(first bytes {hex_bytes(needle[:SIG_MIN])})"
    )


def signature_at(pe, rva: int, cap: int = SIG_CAP, space: bytes | None = None) -> str:
    """Synthesise the published signature string for the function at ``rva``.

    Rule: shortest prefix that is unique in the scan space, but never shorter
    than :data:`SIG_FLOOR`; capped at ``cap`` bytes.
    """
    raw = pe.read(rva, cap)
    if space is None:
        space = scan_space(pe)
    best = shortest_unique_prefix(space, raw)
    if len(best) < SIG_FLOOR:
        trimmed = raw[: min(SIG_FLOOR, cap)]
        if occurrence_count(space, trimmed) != 1:
            raise SigError(
                f"floor length {SIG_FLOOR} is not unique at 0x{rva:X}"
            )
        best = trimmed
    return hex_bytes(best)


def scan_space(pe, sections: tuple[str, ...] = (".text",)) -> bytes:
    """Bytes the runtime scans when matching patterns (code by default)."""
    chunks = []
    for name in sections:
        try:
            sec = pe.section(name)
        except Exception:
            continue
        chunks.append(pe.blob[sec.raw_ptr : sec.raw_ptr + sec.raw_size])
    if not chunks:
        raise SigError("no scan sections found")
    return b"".join(chunks)
