"""Signature synthesis rules.

These tests pin the *exact* upstream convention, which was reverse-verified
against published TOMLs: shortest prefix that is unique in the scan space, but
never shorter than SIG_FLOOR (24 bytes), capped at SIG_CAP (80 bytes).
"""

import pytest

from ost_patterns import sig


class _FakeImage:
    """Minimal stand-in exposing only ``read()`` as used by signature_at()."""

    def __init__(self, data: bytes):
        self.data = data

    def read(self, rva: int, size: int) -> bytes:
        return self.data[rva : rva + size]


def test_hex_roundtrip():
    raw = bytes([0x48, 0x8B, 0xC4])
    assert sig.hex_bytes(raw) == "48 8B C4"
    assert sig.hex_to_bytes("48 8B C4") == raw


def test_floor_pads_short_unique_prefix():
    # `needle` is unique after 4 bytes, but the published sig must be 24 bytes.
    head = bytes(range(0x40, 0x58))
    space = b"\x90" * 64 + head + b"\x90" * 64
    image = _FakeImage(head + bytes(64))
    text = sig.signature_at(image, 0, space=space)
    assert len(text.split()) == sig.SIG_FLOOR
    assert text == sig.hex_bytes(head)


def test_longer_unique_prefix_is_kept():
    # Two copies of a 40-byte block share their first 30 bytes; uniqueness
    # therefore only appears at 31+, which must win over the 24-byte floor.
    block = bytes((i * 7) & 0xFF for i in range(40))
    twin = bytearray(block)
    twin[32] ^= 0xFF
    space = block + b"\x90" * 16 + bytes(twin)
    image = _FakeImage(block + bytes(64))
    text = sig.signature_at(image, 0, space=space)
    assert len(text.split()) == 33


def test_ambiguous_prefix_fails_loudly():
    head = bytes(range(0x40, 0x58))
    space = head + b"\x90" * 8 + head
    image = _FakeImage(head + bytes(64))
    with pytest.raises(sig.SigError):
        sig.shortest_unique_prefix(space, head)


def test_occurrence_count_counts_overlaps():
    assert sig.occurrence_count(b"aaaa", b"aa") == 3
    assert sig.occurrence_count(b"abcabc", b"abc") == 2
