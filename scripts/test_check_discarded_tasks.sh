#!/usr/bin/env bash
#
# Controls for scripts/check_discarded_tasks.sh: each shape the gate claims to
# catch must fail it, and each shape it must leave alone must pass. A grep
# gate that is never shown a violation can go dead and still say "OK".
#
# Usage: scripts/test_check_discarded_tasks.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_discarded_tasks.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0

# run_case <name> <expected: fail|pass> <file body>
run_case() {
  local name="$1" expect="$2" body="$3" tree status
  tree="$TMP/$name"
  mkdir -p "$tree/crates/demo/src" "$tree/scripts"
  printf '%s\n' "$body" > "$tree/crates/demo/src/lib.rs"
  status=0
  DISCARDED_TASKS_ROOT="$tree" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$expect" == fail && $status -ne 1 ]] || [[ "$expect" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $expect, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

# --- positive controls: the gate must flag these -------------------------

run_case let_underscore fail 'fn f() {
    let _ = self.handle_x();
}'

run_case drop_bare fail 'fn f() {
    drop(self.handle_x());
}'

run_case drop_mem fail 'fn f() {
    mem::drop(self.handle_x());
}'

run_case drop_std_mem fail 'fn f() {
    std::mem::drop(self.handle_x());
}'

run_case drop_core_mem fail 'fn f() {
    core::mem::drop(self.handle_x());
}'

run_case drop_wrapped fail 'fn f() {
    drop(
        app.handle_x(1),
    );
}'

run_case drop_annotated_iced_task fail 'fn f() {
    let opened: iced::Task<Message> = make();
    drop(opened);
}'

run_case drop_annotated_task fail 'fn f() {
    let opened: Task = make();
    drop(opened);
}'

run_case drop_wrapped_binding fail 'fn f() {
    let opened = app.handle_x(p);
    drop(
        opened,
    );
}'

run_case drop_task_in_trailing_test_mod fail 'fn f() {}
#[cfg(test)]
mod tests {
    fn t() {
        let opened = app.handle_x();
        drop(opened);
    }
}'

# --- negative controls: the gate must leave these alone ------------------

run_case drop_lock_guard pass 'fn f() {
    let guard = lock.lock();
    drop(guard);
}'

run_case drop_task_id_annotation pass 'fn f() {
    let id: TaskId = make();
    drop(id);
}'

run_case drop_my_task_annotation pass 'fn f() {
    let x: MyTask<u8> = make();
    drop(x);
}'

run_case drop_same_name_other_fn pass 'fn a() {
    let state = app.handle_x();
    state
}
fn b() {
    let state = lock.lock();
    drop(state);
}'

run_case drop_in_trailing_test_mod pass 'fn f() {}
#[cfg(test)]
mod tests {
    fn t() {
        let guard = lock.lock();
        drop(guard);
    }
}'

run_case task_returned pass 'fn f() -> Task<Message> {
    self.handle_x()
}'

if (( failures )); then
  echo "check_discarded_tasks controls: $failures case(s) failed"
  exit 1
fi
echo "check_discarded_tasks controls: OK"
