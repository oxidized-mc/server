#!/usr/bin/env bash
# setup-ref.sh — Download, extract, and decompile the Minecraft 26.1-pre-3 server JAR.
#
# Creates the mc-server-ref/ directory with:
#   extracted/server.jar   — unbundled server JAR (class files)
#   decompiled/            — VineFlower-decompiled Java source (~4 800 files)
#   mc-extracted/          — data-generated registry/tag JSONs
#   generated/             — vanilla data-generator reports (registries.json, etc.)
#
# Prerequisites: java (≥21), curl, jq
# Usage:  ./tools/setup-ref.sh

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────────────
MC_VERSION="26.1-pre-3"
VINEFLOWER_VERSION="1.11.1"
VINEFLOWER_URL="https://github.com/Vineflower/vineflower/releases/download/${VINEFLOWER_VERSION}/vineflower-${VINEFLOWER_VERSION}.jar"
VERSION_MANIFEST_URL="https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REF_DIR="${PROJECT_ROOT}/mc-server-ref"
EXTRACTED_DIR="${REF_DIR}/extracted"
DECOMPILED_DIR="${REF_DIR}/decompiled"
MC_EXTRACTED_DIR="${REF_DIR}/mc-extracted"
GENERATED_DIR="${REF_DIR}/generated"

# ── Helpers ────────────────────────────────────────────────────────────────────
info()  { printf '\033[1;34m==> %s\033[0m\n' "$*"; }
ok()    { printf '\033[1;32m  ✓ %s\033[0m\n' "$*"; }
warn()  { printf '\033[1;33m  ⚠ %s\033[0m\n' "$*"; }
die()   { printf '\033[1;31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not found. Please install it."
}

# ── Preflight checks ──────────────────────────────────────────────────────────
require_cmd java
require_cmd curl
require_cmd jq

JAVA_MAJOR=$(java -version 2>&1 | head -1 | sed -E 's/.*"([0-9]+).*/\1/')
if [ "${JAVA_MAJOR}" -lt 21 ]; then
    die "Java 21+ is required (found Java ${JAVA_MAJOR})"
fi
ok "Java ${JAVA_MAJOR} found"

mkdir -p "${REF_DIR}" "${EXTRACTED_DIR}"

# ── Step 1: Download server JAR ────────────────────────────────────────────────
SERVER_JAR="${REF_DIR}/server.jar"

if [ -f "${SERVER_JAR}" ]; then
    ok "Server JAR already exists, skipping download"
else
    info "Fetching version manifest for ${MC_VERSION}..."
    VERSION_URL=$(curl -sSL "${VERSION_MANIFEST_URL}" \
        | jq -r --arg v "${MC_VERSION}" '.versions[] | select(.id == $v) | .url')

    if [ -z "${VERSION_URL}" ] || [ "${VERSION_URL}" = "null" ]; then
        die "Version '${MC_VERSION}' not found in Mojang manifest"
    fi

    info "Fetching version metadata..."
    SERVER_DOWNLOAD_URL=$(curl -sSL "${VERSION_URL}" \
        | jq -r '.downloads.server.url')

    if [ -z "${SERVER_DOWNLOAD_URL}" ] || [ "${SERVER_DOWNLOAD_URL}" = "null" ]; then
        die "No server download URL found for ${MC_VERSION}"
    fi

    info "Downloading server JAR..."
    curl -#L -o "${SERVER_JAR}" "${SERVER_DOWNLOAD_URL}"
    ok "Downloaded server JAR ($(du -h "${SERVER_JAR}" | cut -f1))"
fi

# ── Step 2: Extract the bundled server JAR ─────────────────────────────────────
INNER_JAR="${EXTRACTED_DIR}/server.jar"

if [ -f "${INNER_JAR}" ]; then
    ok "Extracted server JAR already exists, skipping extraction"
else
    info "Extracting bundled server JAR from META-INF/versions/..."

    # The bundled JAR launcher packs the real server inside META-INF/versions/<hash>/server.jar
    BUNDLED_PATH=$(java -jar "${SERVER_JAR}" --help 2>&1 || true)

    # Use the bundler to extract — run it with a special property
    cd "${EXTRACTED_DIR}"
    java -DbundlerMainClass=net.minecraft.bundler.Main -jar "${SERVER_JAR}" --extract 2>/dev/null || true

    # The extract flag may not exist on all versions. Fallback: manually unzip.
    if [ ! -f "${INNER_JAR}" ]; then
        warn "Bundler extract failed, trying manual extraction..."
        # Unzip META-INF/versions/ to find the inner JAR
        TEMP_EXTRACT=$(mktemp -d)
        cd "${TEMP_EXTRACT}"
        jar xf "${SERVER_JAR}" META-INF/versions/ 2>/dev/null || unzip -q "${SERVER_JAR}" 'META-INF/versions/*' 2>/dev/null || true

        FOUND_JAR=$(find "${TEMP_EXTRACT}" -name 'server-*.jar' -o -name 'server.jar' 2>/dev/null | head -1)
        if [ -n "${FOUND_JAR}" ]; then
            cp "${FOUND_JAR}" "${INNER_JAR}"
        else
            # Some versions just have classes directly — copy the outer JAR
            warn "No inner server JAR found; using outer JAR directly"
            cp "${SERVER_JAR}" "${INNER_JAR}"
        fi
        rm -rf "${TEMP_EXTRACT}"
    fi

    cd "${PROJECT_ROOT}"
    ok "Extracted server JAR ($(du -h "${INNER_JAR}" | cut -f1))"
fi

# ── Step 3: Download VineFlower decompiler ─────────────────────────────────────
VINEFLOWER_JAR="${REF_DIR}/vineflower-${VINEFLOWER_VERSION}.jar"

if [ -f "${VINEFLOWER_JAR}" ]; then
    ok "VineFlower ${VINEFLOWER_VERSION} already exists, skipping download"
else
    info "Downloading VineFlower ${VINEFLOWER_VERSION}..."
    curl -#L -o "${VINEFLOWER_JAR}" "${VINEFLOWER_URL}"
    ok "Downloaded VineFlower ($(du -h "${VINEFLOWER_JAR}" | cut -f1))"
fi

# ── Step 4: Decompile ─────────────────────────────────────────────────────────
if [ -d "${DECOMPILED_DIR}" ] && [ "$(find "${DECOMPILED_DIR}" -name '*.java' | head -1)" ]; then
    JAVA_COUNT=$(find "${DECOMPILED_DIR}" -name '*.java' | wc -l)
    ok "Decompiled directory already has ${JAVA_COUNT} Java files, skipping"
else
    info "Decompiling server JAR with VineFlower (this may take a few minutes)..."
    mkdir -p "${DECOMPILED_DIR}"
    java -jar "${VINEFLOWER_JAR}" \
        -ren=1 -rbr=1 -rsy=1 -din=1 -dgs=1 -den=1 -lit=1 -asc=1 -log=WARN \
        "${INNER_JAR}" "${DECOMPILED_DIR}"

    JAVA_COUNT=$(find "${DECOMPILED_DIR}" -name '*.java' | wc -l)
    ok "Decompiled ${JAVA_COUNT} Java files"
fi

# ── Step 5: Run vanilla data generator ─────────────────────────────────────────
if [ -d "${GENERATED_DIR}/reports" ]; then
    ok "Generated reports already exist, skipping data generation"
else
    info "Running vanilla data generator..."
    mkdir -p "${GENERATED_DIR}"
    cd "${REF_DIR}"
    java -DbundlerMainClass=net.minecraft.data.Main -jar "${SERVER_JAR}" \
        --all --output "${GENERATED_DIR}" 2>&1 | tail -5
    cd "${PROJECT_ROOT}"
    ok "Data generation complete"
fi

# ── Step 6: Extract server data (registries, tags, etc.) ──────────────────────
if [ -d "${MC_EXTRACTED_DIR}/data" ]; then
    ok "Extracted data directory already exists, skipping"
else
    info "Extracting data from server JAR..."
    mkdir -p "${MC_EXTRACTED_DIR}"
    cd "${MC_EXTRACTED_DIR}"
    jar xf "${INNER_JAR}" data/ 2>/dev/null || unzip -qo "${INNER_JAR}" 'data/*' 2>/dev/null || true
    cd "${PROJECT_ROOT}"

    if [ -d "${MC_EXTRACTED_DIR}/data" ]; then
        ok "Extracted data directory"
    else
        warn "No data/ directory found in server JAR (may need manual extraction)"
    fi
fi

# ── Summary ────────────────────────────────────────────────────────────────────
echo ""
info "Setup complete! Directory layout:"
echo ""
echo "  mc-server-ref/"
[ -f "${SERVER_JAR}" ]    && echo "  ├── server.jar              $(du -h "${SERVER_JAR}" | cut -f1)"
[ -f "${INNER_JAR}" ]     && echo "  ├── extracted/server.jar    $(du -h "${INNER_JAR}" | cut -f1)"
[ -f "${VINEFLOWER_JAR}" ] && echo "  ├── vineflower-${VINEFLOWER_VERSION}.jar"
if [ -d "${DECOMPILED_DIR}" ]; then
    JAVA_COUNT=$(find "${DECOMPILED_DIR}" -name '*.java' | wc -l)
    echo "  ├── decompiled/             ${JAVA_COUNT} Java files"
fi
[ -d "${GENERATED_DIR}" ]    && echo "  ├── generated/              vanilla reports"
[ -d "${MC_EXTRACTED_DIR}" ] && echo "  └── mc-extracted/           registry & tag data"
echo ""
ok "Ready to develop! Run 'python3 tools/bundle_registries.py' next if needed."
