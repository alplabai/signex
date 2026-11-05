#!/usr/bin/env bash
#
# Controls for scripts/check_io_ports.sh: each shape the gate must catch fails
# it, and each shape it must leave alone passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_io_ports.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0

# run_case <name> <expected: fail|pass> <relative file> <body>
run_case() {
  local name="$1" expect="$2" file="$3" body="$4" tree status
  tree="$TMP/$name"
  mkdir -p "$tree/$(dirname "$file")"
  printf '%s\n' "$body" > "$tree/$file"
  status=0
  IO_PORTS_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

APP=crates/signex-app/src
run_case rfd_outside fail "$APP/app/menu.rs" 'fn f() { rfd::AsyncFileDialog::new(); }'
run_case webbrowser_outside fail "$APP/about.rs" 'fn f() { webbrowser::open("x"); }'
run_case launcher_outside fail "$APP/opener.rs" 'fn f() { Command::new("xdg-open"); }'
run_case other_crate fail "crates/demo/src/lib.rs" 'fn f() { rfd::FileDialog::new(); }'
run_case in_adapter pass "$APP/io_ports/native.rs" 'fn f() { rfd::AsyncFileDialog::new(); webbrowser::open("x"); }'
run_case doc_comment pass "$APP/app/menu.rs" '/// Runs `rfd::AsyncFileDialog` behind the picker.'
run_case clean pass "$APP/app/menu.rs" 'fn f() { picker.dialog(); }'

if [[ $failures -ne 0 ]]; then
  echo "$failures control(s) failed"
  exit 1
fi
echo "All io-ports gate controls passed."
