"""T183: unit tests for tools/publish_marketplace. No network, no real `gh` — every subprocess
call goes through a fake. Run: `mise exec -- pytest tools/tests` from the repo root, or
`just check` (wired into the `python` recipe).
"""

import json
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from publish_marketplace import PublishError, registry
from publish_marketplace import cli

REPO_ROOT = Path(__file__).resolve().parent.parent.parent


def test_registry_covers_every_plugins_host():
    plugin_dirs = {p.name for p in (REPO_ROOT / "plugins").iterdir() if p.is_dir()}
    assert set(registry.all_hosts()) == plugin_dirs


def test_supported_hosts_are_claude_and_codex():
    assert registry.supported_hosts() == ["claude", "codex"]


def test_unknown_host_raises():
    with pytest.raises(PublishError, match="unknown host"):
        registry.require_supported("not-a-real-host")


def test_unsupported_host_raises_with_reason():
    with pytest.raises(PublishError, match="cursor.*no marketplace publish support"):
        registry.require_supported("cursor")


def test_claude_and_codex_verify_against_real_repo_files():
    for host in registry.SUPPORTED.values():
        host.verify_local(REPO_ROOT)  # raises on failure


def test_verify_local_wrong_entry_path_raises(tmp_path):
    (tmp_path / "plugins" / "fake").mkdir(parents=True)
    (tmp_path / "marketplace.json").write_text(
        json.dumps({"plugins": [{"name": "rtok", "source": "./somewhere/else"}]})
    )
    host = registry.GitCatalogHost("fake", "marketplace.json", "plugins/fake")
    with pytest.raises(PublishError, match="points at"):
        host.verify_local(tmp_path)


def test_dispatch_command_shape():
    assert cli.dispatch_command("claude") == [
        "gh", "workflow", "run", "marketplace.yml", "--repo", "listepo/rtok", "-f", "host=claude",
    ]


def _fake_runner(list_stdout='[{"url": "https://github.com/listepo/rtok/actions/runs/1"}]'):
    calls = []

    def runner(args):
        calls.append(list(args))
        if args[:3] == ["gh", "workflow", "run"]:
            return subprocess.CompletedProcess(args, 0, stdout="", stderr="")
        return subprocess.CompletedProcess(args, 0, stdout=list_stdout, stderr="")

    return runner, calls


def test_trigger_returns_run_url_from_fake_gh():
    runner, calls = _fake_runner()
    assert cli.trigger("claude", runner=runner) == "https://github.com/listepo/rtok/actions/runs/1"
    assert calls[0] == cli.dispatch_command("claude")


def test_cli_dry_run_never_calls_the_runner(capsys):
    def exploding_runner(args):
        raise AssertionError("dry-run must never invoke gh")

    rc = cli.main(["all", "--dry-run"], runner=exploding_runner)
    out = capsys.readouterr().out
    assert rc == 0
    assert "claude" in out and "codex" in out


def test_cli_refuses_unsupported_host(capsys):
    rc = cli.main(["cursor", "--dry-run"])
    assert rc == 1
    assert "no marketplace publish support" in capsys.readouterr().err


def test_cli_real_run_triggers_and_prints_url(capsys):
    runner, _ = _fake_runner()
    rc = cli.main(["claude"], runner=runner)
    assert rc == 0
    assert "claude: https://github.com/listepo/rtok/actions/runs/1" in capsys.readouterr().out
