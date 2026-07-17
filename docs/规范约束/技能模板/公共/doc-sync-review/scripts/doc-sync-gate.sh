#!/bin/sh
set -eu

root=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo "文档一致性审核：当前目录不是 Git 仓库。" >&2
  exit 1
}
cd "$root"

receipt=$(git rev-parse --git-path vibe-doc-sync-review.sha256)
fingerprint=$(git diff --cached --binary --no-ext-diff | git hash-object --stdin)

case "${1:---check}" in
  --record)
    mkdir -p "$(dirname "$receipt")"
    printf '%s\n' "$fingerprint" > "$receipt"
    echo "文档一致性审核已记录当前暂存区。"
    ;;
  --check)
    expected=""
    if [ -f "$receipt" ]; then
      expected=$(cat "$receipt")
    fi
    if [ -z "$expected" ] || [ "$expected" != "$fingerprint" ]; then
      echo "文档一致性审核未完成或凭证已过期。请先运行 doc-sync-review，并执行：" >&2
      echo "  .claude/skills/doc-sync-review/scripts/doc-sync-gate.sh --record" >&2
      exit 1
    fi
    ;;
  *)
    echo "用法：$0 [--record|--check]" >&2
    exit 2
    ;;
esac
