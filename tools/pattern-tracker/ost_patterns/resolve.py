"""Zero-seed rule engine: hook name -> function entry RVA -> signature.

Design rules
------------
*Rules never guess.* Every rule must produce exactly one candidate function; if
it produces zero or several, the hook is reported as unresolved and the run
fails. A silently wrong RVA would disable a hook at best and patch the wrong
function at worst.

Every rule ends in a function **entry point**, which has to come from
``.pdata``. Leaf functions without unwind info (tiny thunks such as
``GetTopManager``) are recovered with a pessimistic backwards scan that only
accepts a start preceded by a return / int3 padding run.

Rule kinds (see ``rules.toml``):

``anchor``
    The function references every literal in ``strings``. Strings are matched
    as exact NUL-terminated ASCII literals in ``.rdata``.
``callers``
    Direct callers of the function resolved by another hook (``of``) or by
    literal anchors (``of_anchor``); candidates can additionally be filtered by
    ``strings`` (literals they reference) and ``span`` (exact byte length).
``callee``
    Direct callees of the same bases; same filters. ``span`` is what makes
    otherwise anonymous leaf helpers (``RepeatedFieldUint32_Add``,
    ``GetOrAddAppData``) identifiable: position in the call graph *plus* an
    exact code length is a strong, cheaply re-verified predicate.
``vtable_slot``
    ``class_name`` + ``slot``: the function installed in that RTTI vtable slot.
    Used for virtuals that are only reachable through the vtable.
``same_as``
    Alias of another hook (the build contains one function under two roles).
"""

from __future__ import annotations

import tomllib
from dataclasses import dataclass, field
from pathlib import Path

from . import pe as pe_mod
from . import rtti as rtti_mod
from . import sig as sig_mod
from .tomlio import PatternEntry


class RuleError(RuntimeError):
    pass


@dataclass
class HookRule:
    name: str
    kind: str
    strings: list[str] = field(default_factory=list)
    of: str | None = None
    of_anchor: list[str] = field(default_factory=list)
    depth: int = 1
    span: int | None = None
    leaf_only: bool = False
    min_callers: int | None = None
    class_name: str | None = None
    slot: int | None = None
    note: str = ""


@dataclass
class Resolved:
    name: str
    rva: int
    via: str


def load_rules(path: str | Path) -> tuple[str, list[HookRule]]:
    """Parse a rules TOML into ``(channel, [HookRule])``."""
    raw = tomllib.loads(Path(path).read_text(encoding="utf-8"))
    channel = raw.get("channel")
    if not isinstance(channel, str):
        raise RuleError(f"{path}: missing top-level `channel`")
    hooks = []
    for item in raw.get("hook", []):
        hooks.append(
            HookRule(
                name=item["name"],
                kind=item["kind"],
                strings=list(item.get("strings", [])),
                of=item.get("of"),
                of_anchor=list(item.get("of_anchor", [])),
                depth=int(item.get("depth", 1)),
                span=None if item.get("span") is None else int(item["span"]),
                leaf_only=bool(item.get("leaf_only", False)),
                min_callers=(
                    None if item.get("min_callers") is None else int(item["min_callers"])
                ),
                class_name=item.get("class_name"),
                slot=None if item.get("slot") is None else int(item["slot"]),
                note=item.get("note", ""),
            )
        )
    if not hooks:
        raise RuleError(f"{path}: no [[hook]] entries")
    return channel, hooks


def heuristic_start(image: pe_mod.PE, rva: int, max_back: int = 0x600) -> int | None:
    """Best-effort entry point for a leaf function that ``.pdata`` omits.

    Walks back to the first byte after the previous function's terminator and
    skips ``int3`` padding. Returns ``None`` unless the result looks sane (i.e.
    the byte before the candidate start is a terminator or padding).
    """
    lo = max(rva - max_back, image.text.va)
    for candidate in range(rva - 1, lo, -1):
        if image.blob[image.offset(candidate)] not in (0xC3, 0xCB, 0xC2, 0xCC):
            continue
        start = candidate + 1
        # Skip the int3 padding run the linker inserts between functions.
        while start < rva and image.blob[image.offset(start)] == 0xCC:
            start += 1
        if start >= rva:
            continue
        return start
    return None


def entry_for(image: pe_mod.PE, insn_rva: int) -> tuple[int, str]:
    """Function entry containing ``insn_rva`` plus how it was derived."""
    fn = image.func_of(insn_rva)
    if fn is not None:
        return fn[0], "pdata"
    start = heuristic_start(image, insn_rva)
    if start is None:
        raise RuleError(f"no function entry for instruction 0x{insn_rva:X}")
    return start, "leaf"


class Resolver:
    def __init__(self, image: pe_mod.PE, hooks: list[HookRule]):
        self.image = image
        self.hooks = {h.name: h for h in hooks}
        self.order = [h.name for h in hooks]
        self.resolved: dict[str, Resolved] = {}
        self._space = sig_mod.scan_space(image)
        self._rtti: rtti_mod.RTTI | None = None

    # -------------------------------------------------------------- primitives

    def funcs_for_strings(self, strings: list[str]) -> dict[int, list[str]]:
        """``{func_entry: [matched strings]}`` for functions referencing literals."""
        per_string: list[set[int]] = []
        for text in strings:
            rvas = self.image.find_cstring(text)
            if not rvas:
                raise RuleError(f"literal not found: {text!r}")
            funcs: set[int] = set()
            for rva in rvas:
                for site in self.image.lea_xrefs().get(rva, ()):
                    fn = self.image.func_of(site)
                    if fn:
                        funcs.add(fn[0])
            if not funcs:
                raise RuleError(f"literal has no code reference: {text!r}")
            per_string.append(funcs)
        common = set.intersection(*per_string)
        return {fn: strings for fn in common}

    def _span_of(self, func_rva: int) -> int | None:
        """Exact ``.pdata`` byte length of a function (``None`` if leaf/no unwind)."""
        fn = self.image.func_at(func_rva)
        return None if fn is None else fn[1] - fn[0]

    def _resolve_one(self, hook: HookRule) -> Resolved:
        if hook.kind == "same_as":
            if not hook.of or hook.of not in self.resolved:
                raise RuleError(f"{hook.name}: kind=same_as needs a resolved `of`")
            base = self.resolved[hook.of]
            return Resolved(hook.name, base.rva, f"same_as {hook.of}")

        if hook.kind == "vtable_slot":
            if not hook.class_name or hook.slot is None:
                raise RuleError(f"{hook.name}: vtable_slot needs class_name + slot")
            if self._rtti is None:
                self._rtti = rtti_mod.RTTI(self.image)
            table = self._rtti.vtables_for_class(hook.class_name)
            if not table:
                raise RuleError(
                    f"{hook.name}: no RTTI vtable for class {hook.class_name!r}"
                )
            cands = {
                vt.functions[hook.slot]
                for vt in table
                if len(vt.functions) > hook.slot
            }
            if len(cands) != 1:
                raise RuleError(
                    f"{hook.name}: class {hook.class_name} has {len(table)} vtables, "
                    f"slot {hook.slot} gave {len(cands)} candidates"
                )
            fn = next(iter(cands))
            return Resolved(
                hook.name, fn, f"vtable {hook.class_name}[{hook.slot}]"
            )

        if hook.kind == "global_thunk":
            if not hook.of and not hook.of_anchor:
                raise RuleError(f"{hook.name}: global_thunk needs `of` or `of_anchor`")
            if hook.of:
                if hook.of not in self.resolved:
                    raise RuleError(f"{hook.name}: global_thunk needs a resolved `of`")
                base = self.resolved[hook.of].rva
                origin = hook.of
            else:
                cands = self.funcs_for_strings(hook.of_anchor)
                if len(cands) != 1:
                    raise RuleError(
                        f"{hook.name}: of_anchor matched {len(cands)} functions"
                    )
                base = next(iter(cands))
                origin = f"anchor {hook.of_anchor[0]!r}"

            fn = self.image.func_at(base)
            if fn is None:
                raise RuleError(f"{hook.name}: base 0x{base:X} has no .pdata entry")
            begin, end = fn
            globals_touched = {
                target
                for target, sites in self.image.rip_refs().items()
                if any(begin <= site < end for site in sites)
            }
            if not globals_touched:
                raise RuleError(f"{hook.name}: base {origin} touches no globals")
            thunks = self.image.getter_thunks()
            matches = {site for site, g in thunks.items() if g in globals_touched}
            if len(matches) != 1:
                raise RuleError(
                    f"{hook.name}: {len(matches)} getter thunks read a global of "
                    f"{origin} ({', '.join(f'0x{c:X}' for c in sorted(matches))})"
                )
            got = next(iter(matches))
            return Resolved(hook.name, got, f"getter thunk for a global of {origin}")

        if hook.kind == "anchor":
            if not hook.strings:
                raise RuleError(f"{hook.name}: anchor rule needs strings")
            cands = self.funcs_for_strings(hook.strings)
            if len(cands) != 1:
                raise RuleError(
                    f"{hook.name}: anchor matched {len(cands)} functions "
                    f"({', '.join(f'0x{c:X}' for c in sorted(cands))})"
                )
            fn = next(iter(cands))
            return Resolved(hook.name, fn, f"anchor {hook.strings[0]!r}")

        if hook.kind in ("callers", "callee"):
            if hook.of:
                if hook.of not in self.resolved:
                    raise RuleError(f"{hook.name}: kind={hook.kind} needs a resolved `of`")
                base = self.resolved[hook.of].rva
                origin = hook.of
            elif hook.of_anchor:
                cands = self.funcs_for_strings(hook.of_anchor)
                if len(cands) != 1:
                    raise RuleError(
                        f"{hook.name}: of_anchor matched {len(cands)} functions"
                    )
                base = next(iter(cands))
                origin = f"anchor {hook.of_anchor[0]!r}"
            else:
                raise RuleError(
                    f"{hook.name}: kind={hook.kind} needs `of` or `of_anchor`"
                )

            frontier = {base}
            for _ in range(max(hook.depth, 1)):
                step: set[int] = set()
                for fn in frontier:
                    if hook.kind == "callers":
                        step.update(self.image.callers_of(fn))
                    else:
                        step.update(self.image.callees_of(fn))
                frontier = step
            cands = set(frontier)
            if hook.strings:
                allowed = set(self.funcs_for_strings(hook.strings))
                cands &= allowed
            if hook.span is not None:
                cands = {c for c in cands if self._span_of(c) == hook.span}
            if hook.leaf_only:
                # Leaf = no .pdata entry (tiny getters/thunks such as
                # GetPipeClient). Deterministic, and ambiguous the moment a
                # second leaf shows up in the same position.
                cands = {c for c in cands if self.image.func_at(c) is None}
            if hook.min_callers is not None:
                # Popularity floor, used only where the runner-up is an order of
                # magnitude away; the rule fails loudly if a second candidate
                # crosses the floor or if the intended one drops below it.
                cands = {
                    c
                    for c in cands
                    if len(self.image.callers_of(c)) >= hook.min_callers
                }
            if len(cands) != 1:
                raise RuleError(
                    f"{hook.name}: {hook.kind} of {origin} matched {len(cands)} "
                    f"functions ({', '.join(f'0x{c:X}' for c in sorted(cands))})"
                )
            fn = next(iter(cands))
            extra = f" span={hook.span}" if hook.span is not None else ""
            if hook.leaf_only:
                extra += " leaf"
            return Resolved(hook.name, fn, f"{hook.kind} of {origin}{extra}")

        raise RuleError(f"{hook.name}: unknown kind {hook.kind!r}")

    # ------------------------------------------------------------------- driver

    def resolve_one(self, name: str) -> PatternEntry:
        """Resolve a single hook (recursively resolving its dependency)."""
        self._ensure(name)
        got = self.resolved[name]
        return PatternEntry(
            name=name,
            rva=got.rva,
            sig=sig_mod.signature_at(self.image, got.rva, space=self._space),
        )

    def _ensure(self, name: str, stack: tuple[str, ...] = ()) -> None:
        if name in self.resolved:
            return
        if name in stack:
            raise RuleError(f"dependency cycle: {' -> '.join(stack + (name,))}")
        if name not in self.hooks:
            raise RuleError(f"no rule for {name}")
        hook = self.hooks[name]
        if hook.of:
            self._ensure(hook.of, stack + (name,))
        self.resolved[name] = self._resolve_one(hook)

    def resolve_all(self) -> list[PatternEntry]:
        entries: list[PatternEntry] = []
        for name in self.order:
            entries.append(self.resolve_one(name))
        return entries

    def explain(self, entries: list[PatternEntry]) -> str:
        lines = []
        for entry in entries:
            lines.append(f"{entry.name:<34} 0x{entry.rva:<10X} {self.resolved[entry.name].via}")
        return "\n".join(lines)
