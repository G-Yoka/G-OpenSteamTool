"""Minimal, dependency-free x64 PE image reader for the pattern pipeline.

Scope is deliberately narrow -- only what zero-seed signature resolution needs:

  * section table + RVA <-> file-offset translation
  * NUL-terminated ASCII string lookup (``.rdata`` anchors)
  * RIP-relative ``lea`` xrefs out of ``.text`` (how x64 code reaches literals)
  * direct ``call`` xrefs (``e8 rel32``)
  * ``.pdata`` function boundaries -- the authoritative source for "which
    function does this instruction belong to", which is what turns an xref into
    a hook RVA.

Nothing here is Steam-specific; it works on any x64 PE image.
"""

from __future__ import annotations

import bisect
import re
import struct
from dataclasses import dataclass
from pathlib import Path


class PEError(RuntimeError):
    pass


@dataclass(frozen=True)
class Section:
    name: str
    va: int
    vsize: int
    raw_ptr: int
    raw_size: int

    @property
    def vend(self) -> int:
        return self.va + max(self.vsize, self.raw_size)

    def contains(self, rva: int) -> bool:
        return self.va <= rva < self.vend


# ``lea reg, [rip + disp32]`` -- modrm with mod=00, rm=101 (RIP-relative).
# Optional REX prefix in 0x40..0x4F; the bare form is included because a few
# encodings carry the REX byte before the range we scan.
_LEA_RE = re.compile(
    rb"(?:[\x40-\x4f]\x8d|\x8d)[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]"
)
_CALL_RE = re.compile(rb"\xe8")
# RIP-relative ``lea``/``mov`` with an optional REX prefix.
_RIP_RE = re.compile(
    rb"(?:[\x40-\x4f][\x8b\x8d]|[\x8b\x8d])[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]"
)
# Exactly ``mov rax, [rip + disp32] ; ret`` -- the 8-byte singleton getters.
_THUNK_RE = re.compile(rb"\x48\x8b\x05....\xc3", re.DOTALL)


class PE:
    """An x64 PE image mapped in memory (the file is read once, up front)."""

    def __init__(self, path: str | Path):
        self.path = Path(path)
        self.blob = self.path.read_bytes()
        if self.blob[:2] != b"MZ":
            raise PEError(f"{self.path}: not an MZ/PE image")
        (e_lfanew,) = struct.unpack_from("<I", self.blob, 0x3C)
        if self.blob[e_lfanew : e_lfanew + 4] != b"PE\0\0":
            raise PEError(f"{self.path}: missing PE signature")
        coff = e_lfanew + 4
        (nsec,) = struct.unpack_from("<H", self.blob, coff + 2)
        (opt_size,) = struct.unpack_from("<H", self.blob, coff + 16)
        opt = coff + 20
        (magic,) = struct.unpack_from("<H", self.blob, opt)
        if magic != 0x20B:
            raise PEError(f"{self.path}: not PE32+ (magic {magic:#x})")
        (self.image_base,) = struct.unpack_from("<Q", self.blob, opt + 24)
        (self.size_of_image,) = struct.unpack_from("<I", self.blob, opt + 56)

        self.sections: list[Section] = []
        sec_off = opt + opt_size
        for i in range(nsec):
            base = sec_off + i * 40
            raw_name = self.blob[base : base + 8].rstrip(b"\0")
            vsize, va, raw_size, raw_ptr = struct.unpack_from("<IIII", self.blob, base + 8)
            self.sections.append(
                Section(
                    name=raw_name.decode("ascii", "replace"),
                    va=va,
                    vsize=vsize,
                    raw_ptr=raw_ptr,
                    raw_size=raw_size,
                )
            )
        self._sec_by_va = sorted(self.sections, key=lambda s: s.va)
        self._sec_starts = [s.va for s in self._sec_by_va]
        self._lea: dict[int, list[int]] | None = None
        self._rip: dict[int, list[int]] | None = None
        self._thunks: dict[int, int] | None = None
        self._call: dict[int, list[int]] | None = None
        self._call_sites: list[tuple[int, int]] | None = None
        self._funcs: list[tuple[int, int]] | None = None
        self._func_starts: list[int] | None = None

    # ------------------------------------------------------------------ basics

    def section(self, name: str) -> Section:
        for sec in self.sections:
            if sec.name == name:
                return sec
        raise PEError(f"{self.path}: no {name} section")

    @property
    def text(self) -> Section:
        return self.section(".text")

    @property
    def rdata(self) -> Section:
        return self.section(".rdata")

    def section_of(self, rva: int) -> Section | None:
        i = bisect.bisect_right(self._sec_starts, rva) - 1
        if i < 0:
            return None
        sec = self._sec_by_va[i]
        return sec if sec.contains(rva) else None

    def offset(self, rva: int) -> int:
        sec = self.section_of(rva)
        if sec is None:
            raise PEError(f"rva {rva:#x} outside any section")
        delta = rva - sec.va
        if delta >= sec.raw_size:
            raise PEError(f"rva {rva:#x} has no file backing (uninitialised tail)")
        return sec.raw_ptr + delta

    def read(self, rva: int, size: int) -> bytes:
        off = self.offset(rva)
        data = self.blob[off : off + size]
        if len(data) != size:
            raise PEError(f"short read at rva {rva:#x} (+{size})")
        return data

    def u32(self, rva: int) -> int:
        return struct.unpack("<I", self.read(rva, 4))[0]

    # ----------------------------------------------------------------- strings

    def cstring(self, rva: int, limit: int = 300) -> str | None:
        """Decode an ASCII literal at ``rva``; ``None`` if it is not printable."""
        sec = self.section_of(rva)
        if sec is None or sec.name not in (".rdata", ".data", ".text"):
            return None
        try:
            off = self.offset(rva)
        except PEError:
            return None
        end = self.blob.find(b"\0", off, off + limit)
        if end < 0 or end == off:
            return None
        raw = self.blob[off:end]
        if not all(0x20 <= b < 0x7F for b in raw):
            return None
        return raw.decode("ascii")

    def find_cstring(self, text: str) -> list[int]:
        """All RVAs of the exact NUL-terminated literal ``text``."""
        needle = text.encode("ascii") + b"\0"
        hits: list[int] = []
        start = 0
        while True:
            idx = self.blob.find(needle, start)
            if idx < 0:
                return hits
            rva = self.rva_of_offset(idx)
            if rva is not None:
                hits.append(rva)
            start = idx + 1

    def rva_of_offset(self, off: int) -> int | None:
        for sec in self.sections:
            if sec.raw_ptr <= off < sec.raw_ptr + sec.raw_size:
                return sec.va + (off - sec.raw_ptr)
        return None

    # ------------------------------------------------------------------ xrefs

    def lea_xrefs(self) -> dict[int, list[int]]:
        """``target_rva -> [instruction rva]`` for RIP-relative ``lea``."""
        if self._lea is None:
            table: dict[int, list[int]] = {}
            text = self.text
            base_off = text.raw_ptr
            blob = self.blob[base_off : base_off + text.raw_size]
            for m in _LEA_RE.finditer(blob):
                start = m.start()
                rex = blob[start] != 0x8D
                disp_at = start + (3 if rex else 2)
                if disp_at + 4 > len(blob):
                    continue
                (disp,) = struct.unpack_from("<i", blob, disp_at)
                insn_rva = text.va + start
                target = insn_rva + (7 if rex else 6) + disp
                if not (0 < target < self.size_of_image):
                    continue
                table.setdefault(target, []).append(insn_rva)
            for v in table.values():
                v.sort()
            self._lea = table
        return self._lea

    def rip_refs(self) -> dict[int, list[int]]:
        """``target_rva -> [instruction rva]`` for RIP-relative ``lea``/``mov``.

        ``lea`` reaches literals, ``mov reg,[rip+X]`` reaches globals; both are
        needed to connect a class-scoped singleton getter to the code that
        asserts on that singleton.
        """
        if self._rip is None:
            table: dict[int, list[int]] = {}
            text = self.text
            blob = self.blob[text.raw_ptr : text.raw_ptr + text.raw_size]
            for m in _RIP_RE.finditer(blob):
                start = m.start()
                rex = blob[start] != 0x8B and blob[start] != 0x8D
                disp_at = start + (3 if rex else 2)
                if disp_at + 4 > len(blob):
                    continue
                (disp,) = struct.unpack_from("<i", blob, disp_at)
                insn_rva = text.va + start
                target = insn_rva + (7 if rex else 6) + disp
                if not (0 < target < self.size_of_image):
                    continue
                table.setdefault(target, []).append(insn_rva)
            for v in table.values():
                v.sort()
            self._rip = table
        return self._rip

    def getter_thunks(self) -> dict[int, int]:
        """``thunk_rva -> global_rva`` for 8-byte ``mov rax,[rip+G]; ret`` stubs.

        These carry no unwind info (so no ``.pdata`` entry), no literal and no
        call edges; the global they read is the only stable handle on them.
        """
        if self._thunks is None:
            table: dict[int, int] = {}
            text = self.text
            blob = self.blob[text.raw_ptr : text.raw_ptr + text.raw_size]
            for m in _THUNK_RE.finditer(blob):
                start = m.start()
                (disp,) = struct.unpack_from("<i", blob, start + 3)
                insn_rva = text.va + start
                table[insn_rva] = insn_rva + 7 + disp
            self._thunks = table
        return self._thunks

    def call_xrefs(self) -> dict[int, list[int]]:
        """``callee_rva -> [call instruction rva]`` for direct ``call rel32``."""
        if self._call is None:
            table: dict[int, list[int]] = {}
            text = self.text
            base_off = text.raw_ptr
            blob = self.blob[base_off : base_off + text.raw_size]
            for m in _CALL_RE.finditer(blob):
                start = m.start()
                if start + 5 > len(blob):
                    continue
                (rel,) = struct.unpack_from("<i", blob, start + 1)
                insn_rva = text.va + start
                callee = insn_rva + 5 + rel
                if not (0 < callee < self.size_of_image):
                    continue
                table.setdefault(callee, []).append(insn_rva)
            for v in table.values():
                v.sort()
            self._call = table
            self._call_sites = sorted(
                (site, callee) for callee, sites in table.items() for site in sites
            )
        return self._call

    # -------------------------------------------------------------- functions

    def funcs(self) -> list[tuple[int, int]]:
        """``[(begin_rva, end_rva)]`` from ``.pdata``, sorted by begin."""
        if self._funcs is None:
            pdata = self.section(".pdata")
            count = pdata.raw_size // 12
            rows: list[tuple[int, int]] = []
            for i in range(count):
                begin, end, _unwind = struct.unpack_from(
                    "<III", self.blob, pdata.raw_ptr + i * 12
                )
                if begin == 0 or end <= begin:
                    continue
                rows.append((begin, end))
            rows.sort()
            self._funcs = rows
            self._func_starts = [b for b, _ in rows]
        return self._funcs

    def func_of(self, rva: int) -> tuple[int, int] | None:
        """Enclosing ``.pdata`` function of ``rva`` (or ``None``).

        ``.pdata`` ranges are non-overlapping in practice; a handful of chained
        entries share a begin, so we probe a few neighbours before giving up.
        """
        self.funcs()
        assert self._func_starts is not None
        i = bisect.bisect_right(self._func_starts, rva) - 1
        for j in range(i, max(i - 4, -1), -1):
            begin, end = self._funcs[j]  # type: ignore[index]
            if begin <= rva < end:
                return (begin, end)
        return None

    def func_at(self, rva: int) -> tuple[int, int] | None:
        """Function whose entry point is exactly ``rva``."""
        self.funcs()
        assert self._func_starts is not None
        i = bisect.bisect_left(self._func_starts, rva)
        if i < len(self._func_starts) and self._func_starts[i] == rva:
            return self._funcs[i]  # type: ignore[index]
        return None

    def strings_in_func(self, begin: int, end: int, min_len: int = 6) -> dict[int, str]:
        """Literals referenced by ``lea`` from inside ``[begin, end)``."""
        lea = self.lea_xrefs()
        out: dict[int, str] = {}
        for target, sites in lea.items():
            if not any(begin <= site < end for site in sites):
                continue
            text = self.cstring(target)
            if text and len(text) >= min_len:
                out[target] = text
        return out

    def funcs_referencing(self, rva: int) -> list[int]:
        """Functions that contain a ``lea`` to ``rva`` (sorted, de-duplicated)."""
        out: list[int] = []
        for site in self.lea_xrefs().get(rva, ()):
            fn = self.func_of(site)
            if fn and (not out or out[-1] != fn[0]):
                out.append(fn[0])
        return sorted(set(out))

    def callers_of(self, func_rva: int) -> list[int]:
        """Functions containing a direct ``call`` to ``func_rva``."""
        out: set[int] = set()
        for site in self.call_xrefs().get(func_rva, ()):
            fn = self.func_of(site)
            if fn:
                out.add(fn[0])
        return sorted(out)

    def callees_of(self, func_rva: int) -> list[int]:
        """Direct ``call`` targets appearing inside ``func_rva``."""
        fn = self.func_at(func_rva)
        if fn is None:
            return []
        begin, end = fn
        self.call_xrefs()
        assert self._call_sites is not None
        lo = bisect.bisect_left(self._call_sites, (begin, -1))
        hi = bisect.bisect_left(self._call_sites, (end, -1))
        return sorted({callee for _site, callee in self._call_sites[lo:hi]})
