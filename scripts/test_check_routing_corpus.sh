#!/usr/bin/env bash
#
# Controls for scripts/check_routing_corpus.sh: each shape the gate must catch
# fails it, and a complete corpus passes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="$ROOT/scripts/check_routing_corpus.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
failures=0

HEADER=$'slug\tupstream\tcommit\tsource_path\tspdx\tlicense_source\tlicense_text\tnotice\tattribution\tnotes\tconverter\tloss_report\tlayers\tsize_class'
SHA=0123456789abcdef0123456789abcdef01234567

# row <slug> <spdx> <notice>
row() {
  printf '%s\thttps://example.org/%s\t%s\tboard.src\t%s\tLICENSE\tLICENSES/%s.txt\t%s\tBy someone\t-\timporter 1\tnone\t2\tS\n' \
    "$1" "$1" "$SHA" "$2" "$2" "$3"
}

# A tree of `count` CERN-OHL-P boards with their rows.
make_tree() {
  local tree="$1" count="$2" i
  mkdir -p "$tree/testdata/routing-corpus/LICENSES/notices"
  printf 'text\n' > "$tree/testdata/routing-corpus/LICENSES/CERN-OHL-P-2.0.txt"
  printf 'text\n' > "$tree/testdata/routing-corpus/LICENSES/MIT.txt"
  printf 'notice\n' > "$tree/testdata/routing-corpus/LICENSES/notices/someone.txt"
  printf '%s\n' "$HEADER" > "$tree/testdata/routing-corpus/MANIFEST.tsv"
  for ((i = 1; i <= count; i++)); do
    row "board-$i" CERN-OHL-P-2.0 - >> "$tree/testdata/routing-corpus/MANIFEST.tsv"
    printf 'format = "snxpcb/1.4"\n' > "$tree/testdata/routing-corpus/board-$i.snxpcb"
  done
}

# expect <name> <fail|pass> — runs the gate on $TMP/<name>.
expect() {
  local name="$1" want="$2" status=0
  ROUTING_CORPUS_ROOT="$TMP/$name" bash "$GATE" >/dev/null 2>&1 || status=$?
  if [[ "$want" == fail && $status -ne 1 ]] || [[ "$want" == pass && $status -ne 0 ]]; then
    echo "FAIL  $name — expected $want, gate exit $status"
    failures=$((failures + 1))
  else
    echo "ok    $name"
  fi
}

C=testdata/routing-corpus

make_tree "$TMP/complete" 50
expect complete pass

make_tree "$TMP/too_few" 49
expect too_few fail

make_tree "$TMP/board_without_row" 50
printf 'x\n' > "$TMP/board_without_row/$C/stray.snxpcb"
expect board_without_row fail

make_tree "$TMP/row_without_board" 50
row board-51 CERN-OHL-P-2.0 - >> "$TMP/row_without_board/$C/MANIFEST.tsv"
expect row_without_board fail

make_tree "$TMP/licence_not_allowed" 50
row board-51 GPL-3.0-only - >> "$TMP/licence_not_allowed/$C/MANIFEST.tsv"
printf 'x\n' > "$TMP/licence_not_allowed/$C/board-51.snxpcb"
printf 'x\n' > "$TMP/licence_not_allowed/$C/LICENSES/GPL-3.0-only.txt"
expect licence_not_allowed fail

make_tree "$TMP/missing_licence_text" 50
row board-51 CC-BY-4.0 - >> "$TMP/missing_licence_text/$C/MANIFEST.tsv"
printf 'x\n' > "$TMP/missing_licence_text/$C/board-51.snxpcb"
expect missing_licence_text fail

make_tree "$TMP/mit_without_notice" 50
row board-51 MIT - >> "$TMP/mit_without_notice/$C/MANIFEST.tsv"
printf 'x\n' > "$TMP/mit_without_notice/$C/board-51.snxpcb"
expect mit_without_notice fail

make_tree "$TMP/mit_with_notice" 50
row board-51 MIT LICENSES/notices/someone.txt >> "$TMP/mit_with_notice/$C/MANIFEST.tsv"
printf 'x\n' > "$TMP/mit_with_notice/$C/board-51.snxpcb"
expect mit_with_notice pass

make_tree "$TMP/missing_notice" 50
row board-51 MIT LICENSES/notices/nobody.txt >> "$TMP/missing_notice/$C/MANIFEST.tsv"
printf 'x\n' > "$TMP/missing_notice/$C/board-51.snxpcb"
expect missing_notice fail

make_tree "$TMP/repeated_slug" 50
row board-50 CERN-OHL-P-2.0 - >> "$TMP/repeated_slug/$C/MANIFEST.tsv"
expect repeated_slug fail

make_tree "$TMP/short_sha" 50
row board-51 CERN-OHL-P-2.0 - | sed "s/$SHA/abc123/" >> "$TMP/short_sha/$C/MANIFEST.tsv"
printf 'x\n' > "$TMP/short_sha/$C/board-51.snxpcb"
expect short_sha fail

make_tree "$TMP/wrong_header" 50
sed -i '1s/^slug/name/' "$TMP/wrong_header/$C/MANIFEST.tsv"
expect wrong_header fail

# A tree without the manifest is an error, never a pass.
mkdir -p "$TMP/no_manifest"
status=0
ROUTING_CORPUS_ROOT="$TMP/no_manifest" bash "$GATE" >/dev/null 2>&1 || status=$?
if [[ $status -ne 2 ]]; then
  echo "FAIL  no_manifest — expected exit 2, gate exit $status"
  failures=$((failures + 1))
else
  echo "ok    no_manifest"
fi

if [[ $failures -ne 0 ]]; then
  echo "Routing corpus gate controls: $failures failed."
  exit 1
fi
echo "Routing corpus gate controls: all passed."
