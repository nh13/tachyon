#!/usr/bin/env bash
# Install the pre-commit hook that runs the same checks CI does.
#
# Kept as a script rather than a committed .git/hooks file because git does not
# version that directory; this makes the hook opt-in and reviewable.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hook="$repo_root/.git/hooks/pre-commit"

if [[ ! -d "$repo_root/.git" ]]; then
    echo "ERROR: $repo_root is not a git checkout." >&2
    exit 1
fi

cat > "$hook" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
echo "pre-commit: cargo ci-fmt"
cargo ci-fmt
echo "pre-commit: cargo ci-lint"
cargo ci-lint
HOOK

chmod +x "$hook"
echo "Installed $hook (runs cargo ci-fmt + cargo ci-lint)."
echo "Skip once with: git commit --no-verify"
