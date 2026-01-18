#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <version | vX.Y.Z>" >&2
  exit 1
fi

version="$1"
version="${version#v}"
tag="v${version}"

if ! [[ "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.+-]+)?$ ]]; then
  echo "Invalid version: ${version}" >&2
  exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Working tree is dirty. Commit or stash changes first." >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
  echo "Tag already exists: ${tag}" >&2
  exit 1
fi

package_info="$(
  python3 - <<'PY'
import re
from pathlib import Path

text = Path("Cargo.toml").read_text()
in_package = False
name = None
version = None
for line in text.splitlines():
    if line.strip().startswith("["):
        in_package = line.strip() == "[package]"
        continue
    if in_package:
        m = re.match(r'^\s*name\s*=\s*"([^"]+)"', line)
        if m:
            name = m.group(1)
        m = re.match(r'^\s*version\s*=\s*"([^"]+)"', line)
        if m:
            version = m.group(1)
        if name and version:
            print(f"{name}\t{version}")
            break
else:
    raise SystemExit("name/version not found in [package]")
PY
)"

package_name="${package_info%%$'\t'*}"
current_version="${package_info#*$'\t'}"

if [[ "${current_version}" != "${version}" ]]; then
  python3 - "${package_name}" "${version}" <<'PY'
import re
import sys
from pathlib import Path

package_name = sys.argv[1]
version = sys.argv[2]

toml_path = Path("Cargo.toml")
lines = toml_path.read_text().splitlines()
in_package = False
updated = False
for i, line in enumerate(lines):
    if line.strip().startswith("["):
        in_package = line.strip() == "[package]"
        continue
    if in_package and re.match(r'^\s*version\s*=\s*"', line):
        lines[i] = f'version = "{version}"'
        updated = True
        break
if not updated:
    raise SystemExit("version not found in [package]")
toml_path.write_text("\n".join(lines) + "\n")

lock_path = Path("Cargo.lock")
if lock_path.exists():
    lines = lock_path.read_text().splitlines()
    in_pkg = False
    updated = False
    for i, line in enumerate(lines):
        if line.strip() == "[[package]]":
            in_pkg = False
            continue
        if line.strip() == f'name = "{package_name}"':
            in_pkg = True
            continue
        if in_pkg and re.match(r'^\s*version\s*=\s*"', line):
            lines[i] = f'version = "{version}"'
            updated = True
            break
    if not updated:
        raise SystemExit(f"{package_name} not found in Cargo.lock")
    lock_path.write_text("\n".join(lines) + "\n")
PY

  git add Cargo.toml Cargo.lock
  git commit -m "chore: release ${tag}"
else
  echo "Cargo.toml already at version ${version}; skipping commit."
fi

git tag -a "${tag}" -m "${tag}"
