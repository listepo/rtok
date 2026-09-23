"""Publish rtok host plugins (`plugins/<host>/`) to their marketplaces via CI.

T183: `python -m publish_marketplace all|<host> [--dry-run]`, run from `tools/`. See
`README.md` in this package for the host table and how to add a host, and
`../../.github/workflows/marketplace.yml` for the `workflow_dispatch` CI job this script
triggers.
"""


class PublishError(Exception):
    """A refused or failed publish: unknown host, unsupported host, or a validation failure
    (catalog file missing/malformed, entry missing, or the live copy does not match)."""
