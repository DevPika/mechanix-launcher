#!/usr/bin/env bash
# Replace one notification over and over with bodies whose runs grow, shrink,
# move their bold and break their markup, to exercise RichText's in-place
# update. Run with the server up: `cargo run -p notifications`.
#
#   markup-update.sh [delay-seconds] [stress-rounds]
#
# Checks: every replace keeps the id, the server survives. Watch the card:
# it must stay in place, and its body must match the line printed for it.
set -euo pipefail

delay=${1:-1}
rounds=${2:-200}

if ! busctl --user status org.freedesktop.Notifications >/dev/null 2>&1; then
    echo "nothing owns org.freedesktop.Notifications; start the server first" >&2
    exit 1
fi
pid=$(busctl --user status org.freedesktop.Notifications | sed -n 's/^PID=//p')

id=$(notify-send -p "Markup" "plain")
echo "id=$id  (server pid $pid)"

replace() {
    local body=$1 got
    got=$(notify-send -p -r "$id" "Markup" "$body")
    if [[ $got != "$id" ]]; then
        echo "FAIL: replace returned id $got, expected $id, body: $body" >&2
        exit 1
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "FAIL: server died after body: $body" >&2
        exit 1
    fi
}

# name | body | what the card should show
steps=(
    "1 → 2 runs|<b>bold</b> then plain|**bold** then plain"
    "same body|<b>bold</b> then plain|nothing changes"
    "bold moves|plain then <b>bold</b>|plain then **bold**"
    "2 → 4 runs|<b>a</b> b <b>c</b> d|**a** b **c** d"
    "only text changes|<b>a</b> x <b>c</b> y|**a** x **c** y"
    "style flips|a <b>b</b> c <b>d</b>|a **b** c **d**"
    "4 → 1 run|all plain again|all plain again"
    "1 → 1, bold|<b>all bold</b>|**all bold**"
    "bold → plain, same text|all bold|all bold (not bold)"
    "nested|<b>b <i>bi</i> b</b> plain|**b bi b** plain (italic not drawn)"
    "underline|<u>under</u> and <b>bold</b>|under and **bold** (underline not drawn)"
    "unclosed tag|<b>Pritam:</b> <i>see you at the sta|**Pritam:** see you at the sta"
    "stray close|x</b>y|xy"
    "crossed|<b><i>x</b>y</i>|xy"
    "lone <|1 < 2 is true|1 < 2 is true"
    "unknown tag|<span>kept</span> text|kept text"
    "multi-byte|<b>héllo</b> 😀 नमस्ते|**héllo** 😀 नमस्ते"
    "empty body||(empty)"
    "back|<b>done</b>|**done**"
)

for step in "${steps[@]}"; do
    IFS='|' read -r name body expect <<<"$step"
    sleep "$delay"
    replace "$body"
    printf '%-24s  %-42s  expect: %s\n' "$name" "$body" "$expect"
done

echo
echo "stress: $rounds replaces, runs 1 → 5 → 1, as fast as the server answers"
for ((i = 0; i < rounds; i++)); do
    n=$((i % 5 + 1))
    body=""
    for ((k = 0; k < n; k++)); do
        if (( (i + k) % 2 )); then body+="<b>w$k</b> "; else body+="w$k "; fi
    done
    replace "$body"
done
replace "<b>stress</b> done"
echo "ok: id stayed $id, server $pid alive; the card should read: stress done"
