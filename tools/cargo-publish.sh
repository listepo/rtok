#!/usr/bin/env bash
# Manual crates.io publish of the workspace crates, in dependency order (cargo orders a
# multi-package publish itself; the order is printed first). No workflow runs this, and
# release-plz.yml does not publish either (release-plz.toml: publish = false).
#
#   tools/cargo-publish.sh --dry-run              # cargo publish --dry-run, nothing uploaded
#   tools/cargo-publish.sh                        # every crate whose Cargo.toml allows it
#   tools/cargo-publish.sh -p rtok-plugin-sdk     # only the named crates (repeatable)
#
# A crate with `publish = false` is never published: flip it in its Cargo.toml (and in
# release-plz.toml) first. Refuses on a dirty tree, a stale Cargo.lock, a path dependency
# without `version =` or with a version that is not the dependency's own, a publishable crate
# that depends on an unpublishable one, a crate without description/license, and — for a real
# publish of the `rtok` crate — a HEAD that is not the `v<version>` release tag.
set -euo pipefail

dry_run=0
packages=()
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry_run=1 ;;
    -p | --package)
      [ $# -ge 2 ] || { echo "cargo-publish: $1 needs a crate name" >&2; exit 2; }
      packages+=("$2")
      shift
      ;;
    -h | --help)
      sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "cargo-publish: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain)" ]; then
  echo "cargo-publish: refusing on a dirty tree; commit or stash first:" >&2
  git status --short >&2
  exit 1
fi

# --locked: a Cargo.lock that does not match Cargo.toml is a refusal, not a silent update.
metadata=$(cargo metadata --format-version 1 --no-deps --locked)

# Prints the crates to publish, dependency first, one per line; problems go to stderr.
plan=$(printf '%s' "$metadata" | node -e '
const meta = JSON.parse(require("fs").readFileSync(0, "utf8"));
const requested = process.argv.slice(1);
const members = new Set(meta.workspace_members);
const pkgs = meta.packages.filter((p) => members.has(p.id));
const byName = new Map(pkgs.map((p) => [p.name, p]));
const publishable = (p) => !(Array.isArray(p.publish) && p.publish.length === 0);
const errors = [];
const names = requested.length ? requested : pkgs.filter(publishable).map((p) => p.name);
for (const name of names) {
  const p = byName.get(name);
  if (!p) errors.push(`${name}: not a workspace crate`);
  else if (!publishable(p)) errors.push(`${name}: publish = false in its Cargo.toml`);
}
const selected = new Set(names.filter((n) => byName.has(n) && publishable(byName.get(n))));
const pathDeps = (p) => p.dependencies.filter((d) => d.path && d.kind !== "dev");
for (const name of selected) {
  const p = byName.get(name);
  if (!p.description) errors.push(`${name}: no description (crates.io requires one)`);
  if (!p.license && !p.license_file) errors.push(`${name}: no license or license-file`);
  for (const d of pathDeps(p)) {
    const dep = byName.get(d.name);
    if (d.req === "*") {
      errors.push(`${name}: path dependency ${d.name} has no version =`);
    } else if (dep && d.req.replace(/^[\^=~]/, "") !== dep.version) {
      errors.push(`${name}: depends on ${d.name} ${d.req}, but the crate is ${dep.version}`);
    }
    if (dep && !publishable(dep)) {
      errors.push(`${name}: depends on ${d.name}, which has publish = false`);
    } else if (dep && !selected.has(d.name)) {
      console.error(`note: ${name} needs ${d.name} ${dep.version} on crates.io already`);
    }
  }
}
const skipped = pkgs.filter((p) => !publishable(p)).map((p) => p.name);
if (!requested.length && skipped.length) console.error(`skipping (publish = false): ${skipped.join(", ")}`);
if (errors.length) {
  for (const e of errors) console.error(`refusing: ${e}`);
  process.exit(1);
}
if (!selected.size) {
  console.error("nothing to publish: every workspace crate has publish = false");
  process.exit(1);
}
const order = [];
const seen = new Set();
const visit = (name) => {
  if (seen.has(name)) return;
  seen.add(name);
  for (const d of pathDeps(byName.get(name))) if (selected.has(d.name)) visit(d.name);
  order.push(name);
};
[...selected].sort().forEach(visit);
console.log(order.map((n) => `${n} ${byName.get(n).version}`).join("\n"));
' "${packages[@]+"${packages[@]}"}")

args=(publish --locked)
echo "publish order:"
while read -r name version; do
  echo "  $name $version"
  args+=(-p "$name")
  if [ "$name" = rtok ] && [ "$dry_run" -eq 0 ] && ! git tag --points-at HEAD | grep -qx "v$version"; then
    echo "cargo-publish: refusing: HEAD is not tagged v$version, the release the rtok crate must match" >&2
    exit 1
  fi
done <<<"$plan"

if [ "$dry_run" -eq 1 ]; then
  args+=(--dry-run)
fi
echo "+ cargo ${args[*]}"
cargo "${args[@]}"
