"""Integration gate: resolver vs. a real Steam build.

Skipped unless both paths are provided, because the DLLs are large and the
ground-truth TOMLs are third-party data that must not live in this repo.

    set OST_DLL=D:\\Steam\\steamui.dll
    set OST_TRUTH=C:\\path\\to\\<sha256>.toml
    set OST_RULES=rules/steamui.toml
    python -m pytest tests/test_integration.py -q
"""

import os
from pathlib import Path

import pytest

from ost_patterns import cli, pe as pe_mod, resolve as resolve_mod, sig as sig_mod, tomlio


def _env():
    dll = os.environ.get("OST_DLL")
    truth = os.environ.get("OST_TRUTH")
    rules = os.environ.get("OST_RULES")
    if not (dll and truth and rules):
        pytest.skip("set OST_DLL, OST_TRUTH and OST_RULES to run this test")
    dll_path, truth_path, rules_path = Path(dll), Path(truth), Path(rules)
    # The ground truth describes exactly one DLL build; Steam updates in the
    # background, so a mismatch means "stale fixture", not a broken resolver.
    if cli.sha256_of(dll_path) not in truth_path.name:
        pytest.skip(
            f"{truth_path.name} does not describe {dll_path.name} "
            f"(sha256 {cli.sha256_of(dll_path)}); re-derive the fixture"
        )
    return dll_path, truth_path, rules_path


def test_rvA_and_sig_match_ground_truth():
    dll, truth_path, rules_path = _env()
    image = pe_mod.PE(dll)
    truth = tomlio.load_pattern(truth_path)
    _channel, hooks = resolve_mod.load_rules(rules_path)
    resolver = resolve_mod.Resolver(image, hooks)
    space = sig_mod.scan_space(image)

    checked = 0
    for name in truth:
        try:
            entry = resolver.resolve_one(name)
        except resolve_mod.RuleError:
            continue  # no rule yet: covered by test_rule_coverage below
        checked += 1
        expected = truth[name]
        assert entry.rva == expected.rva, f"{name}: RVA mismatch"
        assert sig_mod.signature_at(image, entry.rva, space=space) == expected.sig, (
            f"{name}: signature mismatch"
        )
    assert checked > 0, "no hook had a rule; nothing was verified"


def test_rule_set_covers_entire_feed():
    """When the rule file is complete, its hook names must equal the feed's."""
    _dll, truth_path, rules_path = _env()
    truth = tomlio.load_pattern(truth_path)
    _channel, hooks = resolve_mod.load_rules(rules_path)
    have = {h.name for h in hooks}
    missing = sorted(set(truth) - have)
    extra = sorted(have - set(truth))
    assert not missing, f"no rule for: {missing}"
    assert not extra, f"rules for hooks the feed does not carry: {extra}"
