"""MSVC RTTI helpers: map a decorated class name to its vtable(s) and slots.

This is the escape hatch for hooks that cannot be reached by a literal anchor
or a call-graph edge, i.e. virtual functions that are only ever invoked through
their vtable entry (``CSteamUIAppController::RunFrame`` and friends).

x64 MSVC layout used here::

    TypeDescriptor        : vfptr(8) spare(8) name[NUL]
    CompleteObjectLocator : signature(u32) offset(u32) cdOffset(u32)
                            pTypeDescriptor(RVA) pClassDescriptor(RVA) pSelf(RVA)
    vtable                : [-1] -> CompleteObjectLocator (image-base relative)
                            [0..]  -> function pointers (image-base relative)

All pointer fields are read as ``image_base + rva`` values, which is what the
linker writes for a non-ASLR-relocated image on disk.
"""

from __future__ import annotations

import bisect
import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from . import pe as pe_mod

COL_SIZE = 24


@dataclass(frozen=True)
class VTable:
    rva: int
    functions: tuple[int, ...]

    def slot_of(self, func_rva: int) -> int | None:
        for index, fn in enumerate(self.functions):
            if fn == func_rva:
                return index
        return None


class RTTI:
    def __init__(self, image: pe_mod.PE):
        self.image = image
        self._by_name: dict[str, int] | None = None
        self._vtables: list[tuple[int, tuple[int, ...]]] | None = None

    # ------------------------------------------------------------ descriptors

    def type_descriptors(self) -> dict[str, int]:
        """``{decorated name: TypeDescriptor rva}`` for every RTTI type."""
        if self._by_name is None:
            blob = self.image.blob
            table: dict[str, int] = {}
            start = 0
            while True:
                idx = blob.find(b".?AV", start)
                if idx < 0:
                    break
                start = idx + 1
                end = blob.find(b"\0", idx)
                if end < 0:
                    break
                name = blob[idx:end].decode("ascii", "replace")
                rva = self.image.rva_of_offset(idx)
                if rva is not None and rva >= 16:
                    table.setdefault(name, rva - 16)
            self._by_name = table
        return self._by_name

    def find_class(self, name: str) -> list[int]:
        """TypeDescriptor RVAs whose decorated name contains ``name``."""
        hits = []
        for decorated, td in self.type_descriptors().items():
            bare = decorated[4:].rstrip("@")
            if bare == name or bare.startswith(name + "@") or bare == name + "@@":
                hits.append(td)
        return sorted(hits)

    def col_for_descriptor(self, td_rva: int) -> list[int]:
        """CompleteObjectLocator RVAs whose ``pTypeDescriptor`` == ``td_rva``."""
        out = []
        for rva in self._col_rvas():
            try:
                if self.image.u32(rva + 12) == td_rva:
                    out.append(rva)
            except pe_mod.PEError:
                continue
        return out

    # ---------------------------------------------------------------- vtables

    def vtables(self) -> list[tuple[int, tuple[int, ...]]]:
        """All ``(vtable rva, function rvas)`` pairs found in data sections."""
        if self._vtables is None:
            image = self.image
            base = image.image_base
            text = image.text
            col_rvas = self._col_rvas()
            col_vas = {base + rva for rva in col_rvas}
            found: list[tuple[int, tuple[int, ...]]] = []
            for name in (".rdata", ".data"):
                try:
                    sec = image.section(name)
                except pe_mod.PEError:
                    continue
                try:
                    raw = image.blob[sec.raw_ptr : sec.raw_ptr + sec.raw_size]
                except Exception:
                    continue
                for off in range(0, len(raw) - 16, 8):
                    (head,) = struct.unpack_from("<Q", raw, off)
                    if head not in col_vas:
                        continue
                    vtable_rva = sec.va + off + 8
                    funcs: list[int] = []
                    probe = off + 8
                    while probe + 8 <= len(raw):
                        (value,) = struct.unpack_from("<Q", raw, probe)
                        func_rva = value - base if value > base else value
                        if not (text.va <= func_rva < text.va + text.vsize):
                            break
                        funcs.append(func_rva)
                        probe += 8
                    if funcs:
                        found.append((vtable_rva, tuple(funcs)))
            self._vtables = found
        return self._vtables

    def _col_rvas(self) -> list[int]:
        if not hasattr(self, "_cols"):
            image = self.image
            cols: list[int] = []
            for name in (".rdata", ".data"):
                try:
                    sec = image.section(name)
                except pe_mod.PEError:
                    continue
                raw = image.blob[sec.raw_ptr : sec.raw_ptr + sec.raw_size]
                for off in range(0, len(raw) - COL_SIZE, 4):
                    (sig,) = struct.unpack_from("<I", raw, off)
                    # x64 locators use signature 1, x86 uses 0.
                    if sig not in (0, 1):
                        continue
                    (p_type,) = struct.unpack_from("<I", raw, off + 12)
                    # The descriptor may live in a different section than the
                    # locator (.rdata locators point into .data descriptors).
                    if image.section_of(p_type) is None:
                        continue
                    (p_self,) = struct.unpack_from("<I", raw, off + 20)
                    # pSelf must point back at the locator itself; this single
                    # check rejects the many false positives of a naive scan.
                    if p_self != sec.va + off:
                        continue
                    # pTypeDescriptor points at the TypeDescriptor, whose
                    # decorated name starts 16 bytes in (vfptr + spare).
                    try:
                        text = image.cstring(p_type + 16, limit=64)
                    except pe_mod.PEError:
                        continue
                    if not text or not text.startswith(".?A"):
                        continue
                    cols.append(sec.va + off)
            self._cols = cols
        return self._cols

    def vtables_for_class(self, class_name: str) -> list[VTable]:
        descriptors = set(self.find_class(class_name))
        if not descriptors:
            return []
        out: list[VTable] = []
        for vtable_rva, funcs in self.vtables():
            col = struct.unpack(
                "<Q", self.image.read(vtable_rva - 8, 8)
            )[0] - self.image.image_base
            try:
                td = self.image.u32(col + 12)
            except pe_mod.PEError:
                continue
            if td in descriptors:
                out.append(VTable(vtable_rva, funcs))
        return out
