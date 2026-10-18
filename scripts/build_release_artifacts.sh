#!/usr/bin/env bash
# Build every AWEN release artifact into DIST (default: dist/release).
#
#   scripts/build_release_artifacts.sh VERSION [DIST]
#
# Produces:
#   awenctl-VERSION-x86_64-unknown-linux-gnu.tar.gz   release CLI binary (+ LICENSE, README)
#   *.whl                                             awen-native (maturin), awen_py, awen-models,
#                                                     awen-physics, awen-qphotonic, awen-systems,
#                                                     awen-lab, awen-studio (frontend bundled)
#   awen-mini-checkpoint-VERSION.tar.gz               the bundled AWEN-Mini checkpoint
#   sbom/*.cdx.json                                   CycloneDX SBOMs (scripts/generate_sbom.py)
#   awen-VERSION.tar.gz                               git archive of HEAD
#   BUILD-INFO.json                                   toolchains, commit, awenctl rebuild check
#   SHA256SUMS                                        over every file above
#
# Requires: the pinned Rust toolchain, cargo-cyclonedx, Python with
# maturin/build/pip-audit, and Node.js/npm for the Studio frontend.
set -euo pipefail

VERSION="${1:?usage: build_release_artifacts.sh VERSION [DIST]}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$(mkdir -p "${2:-$ROOT/dist/release}" && cd "${2:-$ROOT/dist/release}" && pwd)"
cd "$ROOT"
SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
export SOURCE_DATE_EPOCH
COMMIT="$(git rev-parse HEAD)"
TARGET=x86_64-unknown-linux-gnu
PY="${PYTHON:-python}"
find "$DIST" -mindepth 1 -delete

echo "== awenctl (built twice to check reproducibility)"
build_awenctl() {
  CARGO_TARGET_DIR="$1" RUSTFLAGS="--remap-path-prefix=$ROOT=/awen --remap-path-prefix=$HOME=/home" \
    cargo build --locked --release --manifest-path awen-runtime/Cargo.toml --bin awenctl
}
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
build_awenctl "$WORK/target-a"
build_awenctl "$WORK/target-b"
SHA_A="$(sha256sum "$WORK/target-a/release/awenctl" | cut -d' ' -f1)"
SHA_B="$(sha256sum "$WORK/target-b/release/awenctl" | cut -d' ' -f1)"
REPRODUCIBLE=false
[[ "$SHA_A" == "$SHA_B" ]] && REPRODUCIBLE=true
echo "awenctl rebuild: $SHA_A vs $SHA_B reproducible=$REPRODUCIBLE"
"$WORK/target-a/release/awenctl" --help > /dev/null
PKG="awenctl-$VERSION-$TARGET"
mkdir -p "$WORK/$PKG"
cp "$WORK/target-a/release/awenctl" LICENSE "$WORK/$PKG/"
cp awen-runtime/README.md "$WORK/$PKG/README.md"
tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
  -C "$WORK" -cf - "$PKG" | gzip -n -9 > "$DIST/$PKG.tar.gz"

echo "== awen-native wheel (maturin)"
"$PY" -m maturin build --release --locked --manifest-path awen-native/Cargo.toml \
  --out "$DIST" --interpreter "$PY"

echo "== Studio frontend (bundled into the awen-studio wheel)"
if [[ -d awen-studio ]]; then
  (cd awen-studio/frontend && npm ci --no-audit --no-fund && npm run build)
  rm -rf awen-studio/awen_studio/static
  cp -r awen-studio/frontend/dist awen-studio/awen_studio/static
fi

echo "== pure-Python wheels"
for pkg in awen-ecosystem/python_awen awen-models awen-physics awen-qphotonic awen-systems awen-lab awen-studio; do
  [[ -d "$pkg" ]] || { echo "skip $pkg (not present)"; continue; }
  "$PY" -m build --wheel --no-isolation --outdir "$DIST" "$pkg"
  rm -rf "$pkg/build"
done
rm -rf awen-studio/awen_studio/static

echo "== AWEN-Mini checkpoint"
CK="awen-models/awen_models/checkpoints/awen-mini"
tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
  -C "$(dirname "$CK")" -cf - awen-mini | gzip -n -9 > "$DIST/awen-mini-checkpoint-$VERSION.tar.gz"

echo "== source archive"
git archive --format=tar.gz --prefix="awen-$VERSION/" --output="$DIST/awen-$VERSION.tar.gz" HEAD

echo "== SBOM"
"$PY" scripts/generate_sbom.py --out "$DIST/sbom" --python

echo "== build info"
"$PY" - "$DIST/BUILD-INFO.json" <<PY
import json, platform, subprocess, sys
def out(*cmd):
    try:
        return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout.strip()
    except Exception as error:  # tool missing
        return f"unavailable: {error}"
json.dump({
    "version": "$VERSION",
    "commit": "$COMMIT",
    "source_date_epoch": int("$SOURCE_DATE_EPOCH"),
    "target": "$TARGET",
    "rustc": out("rustc", "--version"),
    "cargo": out("cargo", "--version"),
    "python": platform.python_version(),
    "maturin": out("$PY", "-m", "maturin", "--version"),
    "node": out("node", "--version"),
    "awenctl_rebuild": {"sha256_first": "$SHA_A", "sha256_second": "$SHA_B", "reproducible": $( [[ $REPRODUCIBLE == true ]] && echo True || echo False )},
    "hardware": "none: photonic paths are simulator-only; no GPU used",
}, open(sys.argv[1], "w"), indent=2, sort_keys=True)
PY

echo "== checksums"
(cd "$DIST" && find . -type f ! -name SHA256SUMS -printf '%P\n' | LC_ALL=C sort | xargs sha256sum > SHA256SUMS)
cat "$DIST/SHA256SUMS"
