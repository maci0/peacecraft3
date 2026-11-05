"""Credential-selection tests. Fake fixtures only, no real key, no live call."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness import load_key_in_process, SecretsError


def _raises(fn):
    try:
        fn()
    except SecretsError:
        return
    raise AssertionError("expected SecretsError")


def test_explicit_path_wins_over_env(tmp_path=None):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "key"
        p.write_text("FAKE_PATH_KEY\n")
        assert load_key_in_process(p, environ={"TYPESAFE_API_KEY": "FAKE_ENV_KEY"}) == "FAKE_PATH_KEY"


def test_env_used_when_no_path(tmp_path=None):
    assert load_key_in_process(environ={"TYPESAFE_API_KEY": "  FAKE_ENV_KEY  "}) == "FAKE_ENV_KEY"


def test_missing_everything_raises_naming_env():
    try:
        load_key_in_process(environ={})
        raise AssertionError("expected SecretsError")
    except SecretsError as e:
        assert "TYPESAFE_API_KEY" in str(e)


def test_missing_explicit_path():
    _raises(lambda: load_key_in_process(Path("/nonexistent-xyz/key"), environ={}))


def test_malformed_and_empty():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "key"
        p.write_text("   \n")
        _raises(lambda: load_key_in_process(p))
        p.write_text("KEY=\n")
        _raises(lambda: load_key_in_process(p))
        p.write_text("KEY:   \n")
        _raises(lambda: load_key_in_process(p))
        p.write_text("# only comments\n")
        _raises(lambda: load_key_in_process(p))


def test_parses_env_style_and_bare():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "key"
        for text, want in [("FAKE_KEY_1\n", "FAKE_KEY_1"),
                           ('TYPESAFE_API_KEY="FAKE_KEY_2"\n', "FAKE_KEY_2"),
                           ("TYPESAFE_API_KEY='FAKE_KEY_3'\n", "FAKE_KEY_3"),
                           ("# c\n\nFAKE_KEY_4\n", "FAKE_KEY_4")]:
            p.write_text(text)
            assert load_key_in_process(p) == want


def test_provider_uses_env_via_injected_environ():
    from jev_harness import JevProvider
    # Provider default path goes through load_key_in_process() which reads
    # os.environ; simulate by monkeypatching that binding in jev_provider.
    import jev_harness.jev_provider as jp
    import jev_harness.secrets as sec
    orig = jp.__dict__.get("load_key_in_process")
    # JevProvider imports inside __init__, so patch secrets module attr.
    orig2 = sec.load_key_in_process
    sec.load_key_in_process = lambda path=None, environ=None: "FAKE_INJECTED"
    try:
        p = JevProvider(transport=lambda u, h, b, t: (_ for _ in ()).throw(Exception("stop")))
    finally:
        sec.load_key_in_process = orig2
    assert p._api_key == "FAKE_INJECTED"


def test_provider_forwards_key_path_and_environ_precedence():
    """Env used with no explicit path; explicit path wins. Fake only."""
    import tempfile
    from jev_harness import JevProvider
    with tempfile.TemporaryDirectory() as d:
        kp = Path(d) / "key"
        kp.write_text("FAKE_PATH_KEY\n")
        env_only = JevProvider(environ={"TYPESAFE_API_KEY": "FAKE_ENV_KEY"}, transport=lambda u,h,b,t: (_ for _ in ()).throw(Exception("stop")))
        assert env_only._api_key == "FAKE_ENV_KEY"
        with_path = JevProvider(key_path=kp, environ={"TYPESAFE_API_KEY": "FAKE_ENV_KEY"}, transport=lambda u,h,b,t: (_ for _ in ()).throw(Exception("stop")))
        assert with_path._api_key == "FAKE_PATH_KEY"
