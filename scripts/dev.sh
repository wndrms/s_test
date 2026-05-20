#!/usr/bin/env bash
# 개발 환경: Docker(DB+API+Worker 컨테이너 빌드) + 로컬 trunk(프론트엔드)
# 사용법: ./scripts/dev.sh
# 종료: tmux kill-session -t lumos-dev

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="lumos-dev"
WEB_TARGET="$ROOT_DIR/target/web"

if tmux has-session -t "$SESSION" 2>/dev/null; then
    echo "세션 '$SESSION'이 이미 존재합니다. 재접속합니다."
    echo "새로 시작하려면: tmux kill-session -t $SESSION"
    tmux attach-session -t "$SESSION"
    exit 0
fi

COLS=$(tput cols 2>/dev/null || echo 220)
LINES=$(tput lines 2>/dev/null || echo 50)

tmux new-session -d -s "$SESSION" -x "$COLS" -y "$LINES"

# docker 창 — DB + API + Worker
tmux rename-window -t "$SESSION:0" "docker"
tmux send-keys -t "$SESSION:docker" "cd '$ROOT_DIR' && docker compose -f docker-compose.yml -f docker-compose.local.yml up --build" Enter

# web 창 — 로컬 trunk
tmux new-window -t "$SESSION" -n "web"
tmux send-keys -t "$SESSION:web" "cd '$ROOT_DIR/crates/web' && CARGO_TARGET_DIR='$WEB_TARGET' trunk serve" Enter

tmux select-window -t "$SESSION:docker"
tmux attach-session -t "$SESSION"
