#!/usr/bin/env bash
#
# wasm-size-benchmark.sh — track WASM binary size per contract (issue #1171).
#
# Builds every contract for wasm32-unknown-unknown and records the release
# binary size. Because the repository workspace does not resolve as-is (mixed
# soroban-sdk majors: `market` pins 20.x, every other contract pins 21.x),
# each soroban-sdk major gets its own isolated workspace under
# `target/wasm-size-bench/`. Repository files are never modified.
#
# Usage:
#   scripts/wasm-size-benchmark.sh                 # build + compare vs baseline
#   scripts/wasm-size-benchmark.sh --update        # build + rewrite the baseline
#   scripts/wasm-size-benchmark.sh --contract nft  # limit to one contract (repeatable)
#   scripts/wasm-size-benchmark.sh --threshold 5   # regression threshold in percent
#   scripts/wasm-size-benchmark.sh --help
#
# Exit status: 0 = no regression above the threshold, 1 = regression or a
# contract status change vs the baseline, 2 = usage error.

set -o pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_ROOT="$REPO_ROOT/target/wasm-size-bench"
BASELINE="$REPO_ROOT/docs/contracts/wasm-size-baseline.md"
TARGET="wasm32-unknown-unknown"
THRESHOLD_PCT=10
UPDATE=0
FILTERS=""

usage() {
    cat <<'EOF'
wasm-size-benchmark.sh — measure release WASM binary size per contract.

  (no args)              build every contract and diff against the baseline
  --update               build every contract and rewrite the baseline
  --contract <name>      only this contract (directory or package name, repeatable)
  --threshold <pct>      regression threshold in percent (default 10)
  -h, --help             show this help

Baseline file: docs/contracts/wasm-size-baseline.md
Build sandbox: target/wasm-size-bench/   (safe to delete)
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --update) UPDATE=1; shift ;;
        --contract) FILTERS="$FILTERS $2"; shift 2 ;;
        --threshold) THRESHOLD_PCT="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
done

[ -f "$REPO_ROOT/Cargo.toml" ] || { echo "error: repository root not found" >&2; exit 2; }
[ "$UPDATE" -eq 1 ] && [ -n "${FILTERS# }" ] && {
    echo "error: --update rebuilds the whole baseline; drop --contract" >&2; exit 2;
}
mkdir -p "$WORK_ROOT"

# ── 1. Inventory: directory, package, soroban-sdk major ──────────────────────
INV="$WORK_ROOT/inventory.tsv"
: > "$INV"
for toml in "$REPO_ROOT"/contracts/*/Cargo.toml; do
    dir="$(basename "$(dirname "$toml")")"
    pkg="$(sed -n 's/^name[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$toml" | head -1)"
    [ -n "$pkg" ] || continue
    sdk_line="$(awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && /^soroban-sdk/{print;exit}' "$toml")"
    major="$(printf '%s' "$sdk_line" | sed -n 's/.*version[[:space:]]*=[[:space:]]*"\([0-9][0-9]*\)\..*/\1/p')"
    [ -n "$major" ] || major="$(printf '%s' "$sdk_line" | sed -n 's/.*soroban-sdk[[:space:]]*=[[:space:]]*"\([0-9][0-9]*\)\..*/\1/p')"
    [ -n "$major" ] || major="unknown"
    printf '%s\t%s\t%s\n' "$dir" "$pkg" "$major" >> "$INV"
done
[ -s "$INV" ] || { echo "error: no contracts found" >&2; exit 2; }

# ── 2. Selection ─────────────────────────────────────────────────────────────
SEL="$WORK_ROOT/selected.txt"
: > "$SEL"
if [ -z "${FILTERS# }" ]; then
    cut -f1 "$INV" > "$SEL"
else
    for want in $FILTERS; do
        match="$(awk -F'\t' -v w="$want" '$1==w || $2==w {print $1}' "$INV")"
        [ -n "$match" ] || { echo "error: no contract matches '$want'" >&2; exit 2; }
        printf '%s\n' "$match" >> "$SEL"
    done
fi

# ── 3. Transitive path-dependency closure of the selection ───────────────────
NEED="$WORK_ROOT/need.txt"
# Every path dependency, from any section: cargo needs the directory to exist
# just to load the manifest, even for dev-dependencies.
all_paths() {
    sed -n 's/.*path = "\([^"]*\)".*/\1/p' "$1" | sed 's|^\.\./||'
}

: > "$NEED"
QUEUE="$(cat "$SEL")"
while [ -n "$QUEUE" ]; do
    cur="$(printf '%s\n' "$QUEUE" | head -1)"
    QUEUE="$(printf '%s\n' "$QUEUE" | tail -n +2)"
    [ -n "$cur" ] || continue
    grep -qx "$cur" "$NEED" 2>/dev/null && continue
    printf '%s\n' "$cur" >> "$NEED"
    toml="$REPO_ROOT/contracts/$cur/Cargo.toml"
    [ -f "$toml" ] || continue
    for dep in $(all_paths "$toml"); do
        grep -qx "$dep" "$NEED" 2>/dev/null && continue
        QUEUE="$QUEUE
$dep"
    done
done

# ── 4. One isolated workspace per soroban-sdk major ──────────────────────────
major_of() { awk -F'\t' -v d="$1" '$1==d{print $3}' "$INV"; }

# a crate that does not declare soroban-sdk itself inherits the major of a path dependency
pass=0
while [ "$pass" -lt 5 ]; do
    changed=0
    while read -r d; do
        [ "$(major_of "$d")" = "unknown" ] || continue
        toml="$REPO_ROOT/contracts/$d/Cargo.toml"
        [ -f "$toml" ] || continue
        for dep in $(all_paths "$toml"); do
            dm="$(major_of "$dep")"
            if [ -n "$dm" ] && [ "$dm" != "unknown" ]; then
                awk -F'\t' -v dd="$d" -v mm="$dm" 'BEGIN{OFS="\t"} $1==dd{$3=mm} {print}' "$INV" > "$INV.tmp"
                mv "$INV.tmp" "$INV"
                changed=1
                break
            fi
        done
    done < "$NEED"
    [ "$changed" -eq 1 ] || break
    pass=$((pass + 1))
done

PROFILES="$(awk '/^\[profile\.release\]/{f=1;next} /^\[/{f=0} f{print}' "$REPO_ROOT/Cargo.toml")"
ws_for() { echo "$WORK_ROOT/sdk$1"; }

majors="$(awk -F'\t' 'NR==FNR{need[$1]=1;next} ($1 in need){print $3}' "$NEED" "$INV" | sort -u)"

# drop workspaces for sdk majors that are no longer part of the selection,
# keep everything else (including target/) so incremental builds stay warm
for existing in "$WORK_ROOT"/sdk*; do
    [ -d "$existing" ] || continue
    em="${existing##*sdk}"
    case " $majors " in *" $em "*) ;; *) rm -rf "$existing" ;; esac
done

sync_contract() {
    # mtime-preserving sync so unchanged crates keep their build fingerprints
    if command -v rsync >/dev/null 2>&1; then
        rsync -a --delete "$REPO_ROOT/contracts/$1/" "$ws/contracts/$1/"
    else
        rm -rf "$ws/contracts/$1"
        cp -R "$REPO_ROOT/contracts/$1" "$ws/contracts/"
    fi
}

for major in $majors; do
    ws="$(ws_for "$major")"
    mkdir -p "$ws/contracts"
    {
        echo '[workspace]'
        echo 'resolver = "2"'
        echo 'members = ['
        awk -F'\t' -v m="$major" \
            'NR==FNR{need[$1]=1;next} ($1 in need) && $3==m{printf "    \"contracts/%s\",\n", $1}' \
            "$NEED" "$INV"
        echo ']'
        echo
        printf '%s\n' "$PROFILES"
    } > "$ws/Cargo.toml.new"
    if cmp -s "$ws/Cargo.toml.new" "$ws/Cargo.toml" 2>/dev/null; then
        rm -f "$ws/Cargo.toml.new"
    else
        mv "$ws/Cargo.toml.new" "$ws/Cargo.toml"
    fi
    awk -F'\t' -v m="$major" \
        'NR==FNR{need[$1]=1;next} ($1 in need) && $3==m{print $1}' "$NEED" "$INV" | while read -r d; do
        [ -d "$REPO_ROOT/contracts/$d" ] && sync_contract "$d"
    done
    # prune workspace copies that are no longer members
    for existing in "$ws"/contracts/*; do
        [ -d "$existing" ] || continue
        base="$(basename "$existing")"
        grep -qxF "$base" "$NEED" || rm -rf "$existing"
    done
    if [ -f "$REPO_ROOT/Cargo.lock" ]; then
        if [ ! -f "$ws/Cargo.lock" ] || [ "$REPO_ROOT/Cargo.lock" -nt "$ws/Cargo.lock" ]; then
            cp "$REPO_ROOT/Cargo.lock" "$ws/Cargo.lock"
        fi
    fi
    if [ -f "$REPO_ROOT/rust-toolchain.toml" ]; then
        if [ ! -f "$ws/rust-toolchain.toml" ] || [ "$REPO_ROOT/rust-toolchain.toml" -nt "$ws/rust-toolchain.toml" ]; then
            cp "$REPO_ROOT/rust-toolchain.toml" "$ws/"
        fi
    fi
done

# ── 5. Build and measure ─────────────────────────────────────────────────────
RES="$WORK_ROOT/results.tsv"
: > "$RES"
file_hash() {
    if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
    else sha256sum "$1" | cut -d' ' -f1; fi
}
TOTAL="$(wc -l < "$SEL" | tr -d ' ')"
n=0
while read -r dir; do
    n=$((n + 1))
    pkg="$(awk -F'\t' -v d="$dir" '$1==d{print $2}' "$INV")"
    major="$(awk -F'\t' -v d="$dir" '$1==d{print $3}' "$INV")"
    ws="$(ws_for "$major")"
    wasm="$ws/target/$TARGET/release/$(printf '%s' "$pkg" | tr '-' '_').wasm"
    log="$WORK_ROOT/$dir.build.log"
    if ! grep -q cdylib "$REPO_ROOT/contracts/$dir/Cargo.toml" 2>/dev/null; then
        printf '%s\tskip\t0\t-\tno cdylib crate-type (not a deployable contract)\n' "$dir" >> "$RES"
        echo "[$n/$TOTAL] $dir ($pkg)"
        echo "         skip no cdylib crate-type"
        continue
    fi
    rm -f "$wasm"
    echo "[$n/$TOTAL] $dir ($pkg)"
    start=$(date +%s)
    if (cd "$ws" && cargo build --release --target "$TARGET" -p "$pkg") >"$log" 2>&1; then
        if [ -f "$wasm" ]; then
            size="$(wc -c < "$wasm" | tr -d ' ')"
            printf '%s\tok\t%s\t%s\t-\n' "$dir" "$size" "$(file_hash "$wasm")" >> "$RES"
            echo "         ok   $size bytes"
        else
            printf '%s\tno-wasm\t0\t-\t%s\n' "$dir" "build succeeded but $(printf '%s' "$pkg" | tr '-' '_').wasm was not produced" >> "$RES"
            echo "         FAIL no wasm artifact"
        fi
    else
        reason="$(grep -m1 -E '^error(\[[0-9]+\])?:' "$log" | cut -c1-160)"
        [ -n "$reason" ] || reason="see $log"
        printf '%s\tfailed\t0\t-\t%s\n' "$dir" "$(printf '%s' "$reason" | tr '\t' ' ')" >> "$RES"
        echo "         FAIL $reason"
    fi
    echo "         ($(($(date +%s) - start))s)"
done < "$SEL"

# ── 6. Emit / compare ────────────────────────────────────────────────────────
RUSTC="$(rustc --version 2>/dev/null || echo 'rustc unavailable')"
CARGO_VER="$(cargo --version 2>/dev/null || echo 'cargo unavailable')"
PROFILE_SUMMARY="$(awk '/^\[profile\.release\]/{f=1;next} /^\[/{f=0} f && /=/{gsub(/[ "]/,""); printf "%s, ", $0}' "$REPO_ROOT/Cargo.toml" | sed 's/, $//')"
NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

if [ "$UPDATE" -eq 1 ]; then
    {
        echo "# WASM Binary Size Baseline"
        echo
        echo "> Auto-generated by \`scripts/wasm-size-benchmark.sh --update\` — do not edit by hand."
        echo
        echo "Tracks the release WASM binary size of every contract so unexpected size"
        echo "growth (issue #1171) is caught before it lands."
        echo
        echo "## How to run locally"
        echo
        echo '```bash'
        echo '# build every contract and diff against this baseline'
        echo './scripts/wasm-size-benchmark.sh'
        echo
        echo '# rebuild and rewrite this baseline after an intentional size change'
        echo './scripts/wasm-size-benchmark.sh --update'
        echo
        echo '# benchmark a single contract'
        echo './scripts/wasm-size-benchmark.sh --contract nft'
        echo '```'
        echo
        echo "Builds run inside an isolated workspace under \`target/wasm-size-bench/\`,"
        echo "so repository files are never modified. A non-zero exit status means a"
        echo "contract grew by more than the threshold or changed status."
        echo
        echo "- Generated: $NOW"
        echo "- Toolchain: \`$RUSTC\`"
        echo "- Cargo: \`$CARGO_VER\`"
        echo "- Artifact: \`target/$TARGET/release/<package>.wasm\`"
        echo "- Profile: \`$PROFILE_SUMMARY\`"
        echo "- Regression threshold: ${THRESHOLD_PCT}%"
        echo
        echo "## Results"
        echo
        echo "| Contract | Package | Status | Size (bytes) | Size (KiB) | SHA256 |"
        echo "|---|---|---|---:|---:|---|"
        sort -t'	' -k1,1 "$RES" | while IFS='	' read -r dir status size sha reason; do
            pkg="$(awk -F'\t' -v d="$dir" '$1==d{print $2}' "$INV")"
            if [ "$status" = "ok" ]; then
                printf '| %s | %s | ok | %s | %s | `%s` |\n' "$dir" "$pkg" "$size" \
                    "$(awk -v s="$size" 'BEGIN{printf "%.1f", s/1024}')" "$(printf '%s' "$sha" | cut -c1-16)"
            else
                printf '| %s | %s | %s | — | — | — |\n' "$dir" "$pkg" "$status"
            fi
        done
        echo
        echo "### Build failures"
        echo
        if grep -qE '	(failed|no-wasm)	' "$RES"; then
            echo "These contracts do not currently produce a WASM artifact. The causes are"
            echo "pre-existing in this repository and unrelated to the benchmark itself:"
            echo
            grep -E '	(failed|no-wasm)	' "$RES" | while IFS='	' read -r dir status size sha reason; do
                pkg="$(awk -F'\t' -v d="$dir" '$1==d{print $2}' "$INV")"
                printf -- '- **%s** (%s): %s\n' "$dir" "$pkg" "$reason"
            done
        else
            echo "None — every contract produced a WASM artifact."
        fi
        if grep -q '	skip	' "$RES"; then
            echo
            echo "Not benchmarked (the package has no cdylib crate-type, so it is not a"
            echo "deployable contract):"
            echo
            grep '	skip	' "$RES" | while IFS='	' read -r dir status size sha reason; do
                printf -- '- %s\n' "$dir"
            done
        fi
    } > "$BASELINE"
    echo
    echo "baseline written: $BASELINE"
    exit 0
fi

echo
echo "=== size comparison (threshold: ${THRESHOLD_PCT}%) ==="
if [ ! -f "$BASELINE" ]; then
    echo "no baseline at $BASELINE — run with --update to create one"
    exit 1
fi

# baseline rows: | contract | package | status | size | ... |
BASE="$WORK_ROOT/baseline.tsv"
grep -E '^\| [a-z_]+ \|' "$BASELINE" | sed 's/|/\t/g' | awk -F'\t' '{gsub(/ /,"",$2); gsub(/ /,"",$4); gsub(/ /,"",$5); if($2!="") printf "%s\t%s\t%s\n", $2, $4, $5}' > "$BASE"

printf '%-24s %12s %12s %9s\n' "contract" "baseline" "current" "delta"
REGRESSIONS=0
CHANGES=0
while IFS='	' read -r dir status size sha reason; do
    base_status="$(awk -F'\t' -v d="$dir" '$1==d{print $2}' "$BASE")"
    base_size="$(awk -F'\t' -v d="$dir" '$1==d{print $3}' "$BASE")"
    [ -n "$base_status" ] || base_status="missing"
    if [ "$status" = "ok" ] && [ "$base_status" = "ok" ]; then
        delta="$(awk -v c="$size" -v b="$base_size" 'BEGIN{ if (b<=0) print "n/a"; else printf "%+.1f", (c-b)*100/b }')"
        printf '%-24s %12s %12s %8s%%\n' "$dir" "$base_size" "$size" "$delta"
        if [ "$delta" != "n/a" ] && awk -v d="$delta" -v t="$THRESHOLD_PCT" 'BEGIN{exit !(d>t)}'; then
            echo "   !! size regression above ${THRESHOLD_PCT}%"
            REGRESSIONS=$((REGRESSIONS + 1))
        fi
    elif [ "$status" = "ok" ]; then
        printf '%-24s %12s %12s %9s\n' "$dir" "$base_status" "$size" "new"
        CHANGES=$((CHANGES + 1))
    else
        printf '%-24s %12s %12s\n' "$dir" "$base_status" "$status"
        [ "$status" = "$base_status" ] || CHANGES=$((CHANGES + 1))
    fi
done < "$RES"

echo
if [ "$REGRESSIONS" -gt 0 ] || [ "$CHANGES" -gt 0 ]; then
    echo "$REGRESSIONS regression(s), $CHANGES status change(s) vs baseline"
    exit 1
fi
echo "no regressions vs baseline"
exit 0
