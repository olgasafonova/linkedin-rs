#!/usr/bin/env bash
# Install git hooks for linkedin-rs.
# Run once after cloning: ./scripts/install-hooks.sh

set -euo pipefail

HOOK_DIR="$(git rev-parse --show-toplevel)/.git/hooks"
SCRIPTS_DIR="$(git rev-parse --show-toplevel)/scripts"

# Pre-push hook: PII scan on staged changes
cat > "$HOOK_DIR/pre-push" << 'HOOK'
#!/usr/bin/env bash
# Pre-push hook: scan for LinkedIn PII before pushing.
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"

echo "Running LinkedIn PII scan..."
if ! "$REPO_ROOT/scripts/linkedin-pii-scan.sh"; then
    echo ""
    echo "Push blocked: PII detected in tracked files."
    echo "Fix the findings above, then try again."
    echo "To bypass (emergency only): git push --no-verify"
    exit 1
fi
HOOK

chmod +x "$HOOK_DIR/pre-push"
echo "Installed pre-push hook (PII scan)"
