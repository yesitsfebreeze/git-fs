# git-fs — release helpers
# Run `just` with no args to list recipes.

set shell := ["bash", "-uc"]

plugin := ".claude-plugin/plugin.json"
package := "package.json"

# Default: show available recipes.
default:
    @just --list

# Bump every version field by 0.0.1 (patch), with odometer turnover at 10
# (x.x.9 -> x.(x+1).0, x.9.9 -> (x+1).0.0). Prints "OLD -> NEW".
bump:
    #!/usr/bin/env bash
    set -euo pipefail
    cur=$(grep -m1 '"version"' "{{plugin}}" | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')
    IFS=. read -r major minor patch <<< "$cur"
    patch=$((patch + 1))
    if (( patch >= 10 )); then patch=0; minor=$((minor + 1)); fi
    if (( minor >= 10 )); then minor=0; major=$((major + 1)); fi
    new="${major}.${minor}.${patch}"
    # Replace the exact current version string in both manifests (keeps the
    # plugin + package entries in lockstep).
    sed -i "s/\"version\"\([[:space:]]*\):\([[:space:]]*\)\"${cur}\"/\"version\"\1:\2\"${new}\"/g" "{{plugin}}" "{{package}}"
    node -e 'JSON.parse(require("fs").readFileSync("{{plugin}}"));JSON.parse(require("fs").readFileSync("{{package}}"))'
    echo "${cur} -> ${new}"

# Bump versions, commit everything, and push to the current branch.
push message="":
    #!/usr/bin/env bash
    set -euo pipefail
    line=$(just bump)
    new="${line##*-> }"
    msg="${message:-chore(release): v${new}}"
    git add -A
    git commit -q -m "$msg" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
    git push origin "$(git rev-parse --abbrev-ref HEAD)"
    echo "pushed v${new}"
