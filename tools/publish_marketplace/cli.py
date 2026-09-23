"""Argument parsing, dispatch, and the `gh workflow run` trigger for
`python -m publish_marketplace`. Every `gh` call goes through an injectable `runner` (defaults
to `subprocess.run`) so tests can supply a fake and never touch the network or a real repo.
`--dry-run` must never call `trigger()`.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from typing import Callable, Sequence

from . import PublishError
from . import registry

WORKFLOW_FILE = "marketplace.yml"
REPO = "listepo/rtok"

Runner = Callable[[Sequence[str]], "subprocess.CompletedProcess[str]"]


def default_runner(args: Sequence[str]) -> "subprocess.CompletedProcess[str]":
    return subprocess.run(args, capture_output=True, text=True, check=False)  # noqa: S603


def dispatch_command(host: str) -> list[str]:
    return ["gh", "workflow", "run", WORKFLOW_FILE, "--repo", REPO, "-f", f"host={host}"]


def _list_command() -> list[str]:
    return ["gh", "run", "list", "--repo", REPO, "--workflow", WORKFLOW_FILE, "--limit", "1", "--json", "url"]


def trigger(host: str, runner: Runner = default_runner) -> str:
    """Fire the workflow_dispatch for `host` and return the triggered run's URL."""
    result = runner(dispatch_command(host))
    if result.returncode != 0:
        raise PublishError(f"gh workflow run failed for host {host!r}: {result.stderr.strip()}")
    result = runner(_list_command())
    if result.returncode != 0:
        raise PublishError(f"gh run list failed for host {host!r}: {result.stderr.strip()}")
    try:
        rows = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise PublishError(f"gh run list returned non-JSON output: {exc}") from exc
    if not rows:
        raise PublishError(f"gh run list returned no runs for workflow {WORKFLOW_FILE!r}")
    return rows[0]["url"]


def _hosts_for(host_arg: str) -> list[str]:
    if host_arg == "all":
        return registry.supported_hosts()
    registry.require_supported(host_arg)  # raises on unknown/unsupported
    return [host_arg]


def main(argv: Sequence[str] | None = None, runner: Runner = default_runner) -> int:
    parser = argparse.ArgumentParser(prog="publish_marketplace")
    parser.add_argument("host", help="'all' or a single host id (see registry.py)")
    parser.add_argument("--dry-run", action="store_true", help="print the plan; fires nothing")
    args = parser.parse_args(argv)

    try:
        hosts = _hosts_for(args.host)
        if args.dry_run:
            print(f"dry-run: {len(hosts)} host(s), nothing fired")
            for host in hosts:
                print(f"  {host}: {dispatch_command(host)}")
            return 0
        for host in hosts:
            print(f"{host}: {trigger(host, runner=runner)}")
    except PublishError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0
