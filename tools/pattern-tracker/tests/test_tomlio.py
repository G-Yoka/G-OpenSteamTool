"""TOML round-trip and rule-parsing behaviour."""

import textwrap

from ost_patterns import resolve as resolve_mod
from ost_patterns import tomlio


def test_pattern_roundtrip(tmp_path):
    entries = [
        tomlio.PatternEntry(name="Zeta", rva=0x1000, sig="90 90"),
        tomlio.PatternEntry(name="Alpha", rva=0xA14DF0, sig="48 8B C4"),
    ]
    text = tomlio.dump_pattern(entries)
    assert "MACHINE-GENERATED" in text
    path = tmp_path / "x.toml"
    path.write_text(text, encoding="utf-8")

    loaded = tomlio.load_pattern(path)
    assert set(loaded) == {"Zeta", "Alpha"}
    assert loaded["Alpha"].rva == 0xA14DF0
    assert loaded["Alpha"].sig == "48 8B C4"
    # Sorted by name so diffs stay stable.
    assert text.index('"Alpha"') < text.index('"Zeta"')


def test_hook_key_is_stable():
    assert tomlio.hook_key("SpawnProcess") == tomlio.hook_key("SpawnProcess")
    assert tomlio.hook_key("SpawnProcess") != tomlio.hook_key("RecvPkt")
    assert tomlio.hook_key("x").startswith("0x")


def test_hook_key_matches_loader():
    """Keys are what OpenSteamTool.dll looks patterns up by (FNV-1a 32).

    Taken from the loader's own log lines, e.g.
    ``PatternLoader: no entry for 'GetAppByID' (key=0x3FC68546)`` and from the
    published feeds. A regression here silently disables every hook.
    """
    known = {
        "GetAppByID": "0x3FC68546",
        "MarkAppChange": "0xC7D5CACF",
        "SendCallbackToPipe": "0x68211B4D",
        "LoadDepotDecryptionKey": "0xB13C0C3F",
        "GetPipeClient": "0x02DF23BC",
        "IPCProcessMessage": "0xC3E20E29",
        "BuildSpawnEnvBlock": "0xDB78B4AE",
        "RecvPkt": "0x836FF9F0",
        "CUtlMemoryGrow": "0x2D945919",
        "AddProtobufAsBinary": "0xD05E26A2",
        "FillInAppOverview": "0xB030A061",
        "SpawnProcess": "0x7D1EC415",
    }
    for name, expected in known.items():
        assert tomlio.hook_key(name) == expected, name



def test_load_rules_requires_channel(tmp_path):
    path = tmp_path / "r.toml"
    path.write_text(
        textwrap.dedent(
            """
            [[hook]]
            name = "A"
            kind = "anchor"
            strings = ["x"]
            """
        ),
        encoding="utf-8",
    )
    try:
        resolve_mod.load_rules(path)
    except resolve_mod.RuleError as exc:
        assert "channel" in str(exc)
    else:  # pragma: no cover
        raise AssertionError("missing channel must be rejected")


def test_load_rules_parses_vtable_slot(tmp_path):
    path = tmp_path / "r.toml"
    path.write_text(
        textwrap.dedent(
            """
            channel = "steamui"

            [[hook]]
            name = "RunFrame"
            kind = "vtable_slot"
            class_name = "CSteamUIAppController"
            slot = 2
            """
        ),
        encoding="utf-8",
    )
    channel, hooks = resolve_mod.load_rules(path)
    assert channel == "steamui"
    assert hooks[0].class_name == "CSteamUIAppController"
    assert hooks[0].slot == 2


def test_load_rules_parses_filters(tmp_path):
    path = tmp_path / "r.toml"
    path.write_text(
        textwrap.dedent(
            """
            channel = "steamclient"

            [[hook]]
            name = "GetPipeClient"
            kind = "callee"
            of = "SendCallbackToPipe"
            leaf_only = true

            [[hook]]
            name = "GetAppIDForCurrentPipe"
            kind = "callee"
            of_anchor = ["CDN anchor literal"]
            leaf_only = true
            min_callers = 100

            [[hook]]
            name = "GetTopManager"
            kind = "global_thunk"
            of_anchor = ["g_pSteamUIController == nullptr"]
            """
        ),
        encoding="utf-8",
    )
    _channel, hooks = resolve_mod.load_rules(path)
    by_name = {h.name: h for h in hooks}
    assert by_name["GetPipeClient"].leaf_only is True
    assert by_name["GetPipeClient"].span is None
    assert by_name["GetAppIDForCurrentPipe"].min_callers == 100
    assert by_name["GetAppIDForCurrentPipe"].of_anchor == ["CDN anchor literal"]
    assert by_name["GetTopManager"].kind == "global_thunk"
    assert by_name["GetTopManager"].of_anchor == ["g_pSteamUIController == nullptr"]

