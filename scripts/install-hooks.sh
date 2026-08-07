#!/usr/bin/env bash
# Install the pre-commit hook that runs the same checks CI does.
#
# Kept as a script rather than a committed .git/hooks file because git does not
# version that directory; this makes the hook opt-in and reviewable.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Ask git for the hooks directory rather than assuming "$repo_root/.git/hooks":
# in a linked worktree .git is a FILE holding a gitdir: pointer, so the literal
# path does not exist and the install would wrongly report "not a git checkout".
if ! hooks_dir="$(git -C "$repo_root" rev-parse --path-format=absolute --git-path hooks 2>/dev/null)"; then
    echo "ERROR: $repo_root is not a git checkout." >&2
    exit 1
fi
mkdir -p "$hooks_dir"
hook="$hooks_dir/pre-commit"

if [[ -e "$hook" ]]; then
    echo "WARNING: overwriting the existing hook at $hook" >&2
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
