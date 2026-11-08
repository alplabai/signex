#!/usr/bin/env bash
#
# Controls for scripts/check_raw_labels.sh: each shape the gate claims to count
# must raise the count, and each shape it must leave alone must not. A grep
# gate that is never shown a violation can go dead and still say "OK".
#
# Usage: scripts/test_check_raw_labels.sh
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_raw_labels.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0

# put <tree> <relative path> <body>
put() {
  mkdir -p "$(dirname "$1/$2")"
  printf '%s\n' "$3" > "$1/$2"
}

# new_tree <name> [baseline] — a tree with that baseline (default 0); echoes its path.
new_tree() {
  local tree="$TMP/$1"
  mkdir -p "$tree/scripts" "$tree/crates"
  put "$tree" scripts/raw-labels-baseline.txt "raw_labels=${2:-0}"
  echo "$tree"
}

# expect <name> <fail|pass> <tree> [gate args]
expect() {
  local name="$1" want="$2" tree="$3" status=0
  shift 3
  RAW_LABELS_ROOT="$tree" bash "$GATE" "$@" >/dev/null 2>&1 || status=$?
  if [[ "$want" == fail && $status -ne 1 ]] || [[ "$want" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $want, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

# --- positive controls: the gate must count these ------------------------

t="$(new_tree call_in_function)"
put "$t" crates/signex-app/src/x.rs 'fn v() { text(raw("Save")); }'
expect call_in_function fail "$t"

t="$(new_tree call_in_macro_token_tree)"
put "$t" crates/signex-app/src/x.rs 'fn v() { row![text(raw("Save")), text(raw(name))]; }'
expect call_in_macro_token_tree fail "$t" # two sites, baseline 0

t="$(new_tree qualified_signex_widgets)"
put "$t" crates/signex-widgets/src/x.rs 'fn v() { signex_widgets::text(signex_widgets::raw("Save")); }'
expect qualified_signex_widgets fail "$t"

t="$(new_tree qualified_crate)"
put "$t" crates/chrome-catalog/src/x.rs 'fn v() { crate::text(crate::raw("Save")); }'
expect qualified_crate fail "$t"

t="$(new_tree grew_past_baseline)" # one site, baseline 0
put "$t" crates/signex-app/src/x.rs 'fn v() { text_input(raw("0"), value); }'
expect grew_past_baseline fail "$t"

t="$(new_tree missing_baseline)"
rm "$t/scripts/raw-labels-baseline.txt"
expect missing_baseline fail "$t"

# --- negative controls: the gate must leave these alone ------------------

t="$(new_tree not_a_call_site)"
put "$t" crates/signex-app/src/a.rs '// text(raw("in a comment"))
/// text(raw("in a doc comment"))
fn raw(pos: u8) -> u8 { pos }
fn v(x: Foo) { x.raw(); Foo::raw(1); from_raw(2); my_raw(3); }'
put "$t" crates/signex-app/tests/b.rs 'fn raw(a: &App, n: &str) {} fn t() { raw(&app, "x"); }'
put "$t" crates/signex-widgets/src/label.rs 'pub fn v() { raw("defined here") }'
put "$t" crates/signex-types/src/c.rs 'fn v() { raw("not a UI crate") }'
expect not_a_call_site pass "$t"

t="$(new_tree baseline_matches 2)"
put "$t" crates/signex-app/src/x.rs 'fn v() { text(raw("a")); text(raw("b")); }'
expect baseline_matches pass "$t"

t="$(new_tree shrank_below_baseline_fails 5)"
put "$t" crates/signex-app/src/x.rs 'fn v() { text(raw("a")); }'
expect shrank_below_baseline_fails fail "$t"

# a drop is accepted once the baseline is lowered in the same change
expect shrank_then_generate pass "$t" --generate
expect shrank_after_generate pass "$t"

# --- bare string literal in a layout macro --------------------------------

t="$(new_tree column_with_literal)"
put "$t" crates/signex-app/src/x.rs 'fn v() { column!["Save"]; }'
expect column_with_literal fail "$t"

t="$(new_tree row_with_trailing_literal)"
put "$t" crates/signex-app/src/x.rs 'fn v() { row![icon(), "Save"]; }'
expect row_with_trailing_literal fail "$t"

t="$(new_tree stack_with_literal_after_comment)"
put "$t" crates/signex-app/src/x.rs 'fn v() { stack![
    base(),
    // the label
    "Save",
]; }'
expect stack_with_literal_after_comment fail "$t"

t="$(new_tree nested_macro_literal)"
put "$t" crates/signex-app/src/x.rs 'fn v() { column![row![a(), b("x, y"), "Save"]]; }'
expect nested_macro_literal fail "$t"

t="$(new_tree macro_elements_that_are_not_bare_literals 1)"
put "$t" crates/signex-app/src/x.rs 'fn v() { column![text(raw("a")), button("b").x(), c("],"), format!("{}", 1).into(), "a".len()]; row![]; vec!["x"]; }
#[cfg(test)]
mod tests { fn t() { column!["in a test module"]; } }'
put "$t" crates/signex-app/src/y_tests.rs 'fn t() { column!["in a tests file"]; }'
expect macro_elements_that_are_not_bare_literals pass "$t"

t="$(new_tree test_mod_declaration_does_not_stop_the_scan)"
put "$t" crates/signex-app/src/x.rs 'mod a;
#[cfg(test)]
mod a_tests;
fn v() { column!["LEAK"]; }'
expect test_mod_declaration_does_not_stop_the_scan fail "$t"

t="$(new_tree url_in_string_is_not_a_comment)"
put "$t" crates/signex-app/src/x.rs 'fn v() { column!["https://x.example"]; }'
expect url_in_string_is_not_a_comment fail "$t"

t="$(new_tree block_comment_before_literal)"
put "$t" crates/signex-app/src/x.rs 'fn v() { column![/* c */ "X"]; }'
expect block_comment_before_literal fail "$t"

t="$(new_tree raw_string_element)"
put "$t" crates/signex-app/src/x.rs 'fn v() { column![r"X"]; }'
expect raw_string_element fail "$t"

t="$(new_tree hashed_raw_string_element)"
put "$t" crates/signex-app/src/x.rs 'fn v() { row![a(), r#"X "q" ]"#]; }'
expect hashed_raw_string_element fail "$t"

t="$(new_tree comments_and_raw_strings_that_are_not_elements 0)"
put "$t" crates/signex-app/src/x.rs 'fn v() { column![/* "X" */ a(), // "Y"
b(r"x").y(), c(r#"]"#)]; }'
expect comments_and_raw_strings_that_are_not_elements pass "$t"

# --generate rewrites the baseline so a grown tree passes again.
t="$(new_tree generate_relocks)"
put "$t" crates/signex-app/src/x.rs 'fn v() { text(raw("a")); }'
expect generate_before fail "$t"
expect generate_run pass "$t" --generate
expect generate_after pass "$t"

if (( failures )); then
  echo "check_raw_labels controls: $failures case(s) failed"
  exit 1
fi
echo "check_raw_labels controls: OK"
