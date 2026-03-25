#!/usr/bin/env bash
# diff-versions.sh — Compare two decompiled Minecraft server versions.
#
# Produces a summary of:
#   - Files added / removed / modified
#   - Per-file diff statistics (lines added/removed)
#   - Categorized changes by Minecraft package (network, world, server, etc.)
#   - Full unified diffs written to an output file
#
# Usage:
#   ./tools/diff-versions.sh [OLD_VERSION] [NEW_VERSION]
#
# Defaults: OLD_VERSION=26.1-pre-3  NEW_VERSION=26.1
# Output:   mc-server-ref/diff-<old>-vs-<new>/

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────────────
OLD_VERSION="${1:-26.1-pre-3}"
NEW_VERSION="${2:-26.1}"

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REF_DIR="${PROJECT_ROOT}/mc-server-ref"
OLD_DIR="${REF_DIR}/${OLD_VERSION}/decompiled"
NEW_DIR="${REF_DIR}/${NEW_VERSION}/decompiled"
OUTPUT_DIR="${REF_DIR}/diff-${OLD_VERSION}-vs-${NEW_VERSION}"

# ── Helpers ────────────────────────────────────────────────────────────────────
info()  { printf '\033[1;34m==> %s\033[0m\n' "$*"; }
ok()    { printf '\033[1;32m  ✓ %s\033[0m\n' "$*"; }
warn()  { printf '\033[1;33m  ⚠ %s\033[0m\n' "$*"; }
die()   { printf '\033[1;31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

bold()  { printf '\033[1m%s\033[0m' "$*"; }
red()   { printf '\033[1;31m%s\033[0m' "$*"; }
green() { printf '\033[1;32m%s\033[0m' "$*"; }
cyan()  { printf '\033[1;36m%s\033[0m' "$*"; }

# ── Preflight ──────────────────────────────────────────────────────────────────
[ -d "${OLD_DIR}" ] || die "Old version directory not found: ${OLD_DIR}"
[ -d "${NEW_DIR}" ] || die "New version directory not found: ${NEW_DIR}"

mkdir -p "${OUTPUT_DIR}"

info "Comparing $(bold "${OLD_VERSION}") → $(bold "${NEW_VERSION}")"
echo ""

# ── Build file lists (relative paths) ─────────────────────────────────────────
OLD_FILES=$(mktemp)
NEW_FILES=$(mktemp)
trap 'rm -f "${OLD_FILES}" "${NEW_FILES}"' EXIT

(cd "${OLD_DIR}" && find . -name '*.java' | sort) > "${OLD_FILES}"
(cd "${NEW_DIR}" && find . -name '*.java' | sort) > "${NEW_FILES}"

OLD_COUNT=$(wc -l < "${OLD_FILES}")
NEW_COUNT=$(wc -l < "${NEW_FILES}")

# ── Classify files ─────────────────────────────────────────────────────────────
ADDED_FILES=$(comm -13 "${OLD_FILES}" "${NEW_FILES}")
REMOVED_FILES=$(comm -23 "${OLD_FILES}" "${NEW_FILES}")
COMMON_FILES=$(comm -12 "${OLD_FILES}" "${NEW_FILES}")

ADDED_COUNT=$(echo "${ADDED_FILES}" | grep -c '.' || true)
REMOVED_COUNT=$(echo "${REMOVED_FILES}" | grep -c '.' || true)
COMMON_COUNT=$(echo "${COMMON_FILES}" | grep -c '.' || true)

# ── Find modified files among common files ─────────────────────────────────────
MODIFIED_FILES=""
MODIFIED_COUNT=0
UNCHANGED_COUNT=0

info "Comparing ${COMMON_COUNT} common files..."

# Pre-create output files
FULL_DIFF="${OUTPUT_DIR}/full.diff"
> "${FULL_DIFF}"

SUMMARY_FILE="${OUTPUT_DIR}/summary.txt"
MODIFIED_LIST="${OUTPUT_DIR}/modified-files.txt"
ADDED_LIST="${OUTPUT_DIR}/added-files.txt"
REMOVED_LIST="${OUTPUT_DIR}/removed-files.txt"
STATS_FILE="${OUTPUT_DIR}/change-stats.txt"
CATEGORY_FILE="${OUTPUT_DIR}/changes-by-category.txt"

> "${MODIFIED_LIST}"
> "${STATS_FILE}"

declare -A CATEGORY_ADDED
declare -A CATEGORY_REMOVED
declare -A CATEGORY_MODIFIED
declare -A CATEGORY_FILES

while IFS= read -r relpath; do
    [ -z "${relpath}" ] && continue

    old_file="${OLD_DIR}/${relpath}"
    new_file="${NEW_DIR}/${relpath}"

    if ! diff -q "${old_file}" "${new_file}" > /dev/null 2>&1; then
        MODIFIED_COUNT=$((MODIFIED_COUNT + 1))

        # Generate unified diff
        file_diff=$(diff -u "${old_file}" "${new_file}" \
            --label "a/${OLD_VERSION}/${relpath}" \
            --label "b/${NEW_VERSION}/${relpath}" 2>/dev/null || true)

        echo "${file_diff}" >> "${FULL_DIFF}"
        echo "" >> "${FULL_DIFF}"

        # Count additions/deletions (skip diff headers)
        lines_added=$(echo "${file_diff}" | grep -c '^+[^+]' || true)
        lines_removed=$(echo "${file_diff}" | grep -c '^-[^-]' || true)

        echo "${relpath}" >> "${MODIFIED_LIST}"
        printf "%-80s  +%-5d -%d\n" "${relpath}" "${lines_added}" "${lines_removed}" >> "${STATS_FILE}"

        # Categorize by top-level package under net/minecraft/
        category=$(echo "${relpath}" | sed -n 's|^\./net/minecraft/\([^/]*\)/.*|\1|p')
        if [ -z "${category}" ]; then
            category="(root)"
        fi

        CATEGORY_ADDED["${category}"]=$(( ${CATEGORY_ADDED["${category}"]:-0} + lines_added ))
        CATEGORY_REMOVED["${category}"]=$(( ${CATEGORY_REMOVED["${category}"]:-0} + lines_removed ))
        CATEGORY_MODIFIED["${category}"]=$(( ${CATEGORY_MODIFIED["${category}"]:-0} + 1 ))
        CATEGORY_FILES["${category}"]="${CATEGORY_FILES["${category}"]:-}${relpath}\n"
    else
        UNCHANGED_COUNT=$((UNCHANGED_COUNT + 1))
    fi
done <<< "${COMMON_FILES}"

# ── Write added/removed lists ─────────────────────────────────────────────────
echo "${ADDED_FILES}" | grep '.' > "${ADDED_LIST}" 2>/dev/null || true
echo "${REMOVED_FILES}" | grep '.' > "${REMOVED_LIST}" 2>/dev/null || true

# Also generate diffs for added files (entire file is new)
if [ -n "${ADDED_FILES}" ]; then
    while IFS= read -r relpath; do
        [ -z "${relpath}" ] && continue
        diff -u /dev/null "${NEW_DIR}/${relpath}" \
            --label "/dev/null" \
            --label "b/${NEW_VERSION}/${relpath}" >> "${FULL_DIFF}" 2>/dev/null || true
        echo "" >> "${FULL_DIFF}"

        category=$(echo "${relpath}" | sed -n 's|^\./net/minecraft/\([^/]*\)/.*|\1|p')
        [ -z "${category}" ] && category="(root)"
        CATEGORY_ADDED["${category}"]=$(( ${CATEGORY_ADDED["${category}"]:-0} + $(wc -l < "${NEW_DIR}/${relpath}") ))
    done <<< "${ADDED_FILES}"
fi

# ── Changes by category ───────────────────────────────────────────────────────
{
    printf "%-25s  %8s  %8s  %8s  %8s\n" "CATEGORY" "MODIFIED" "ADDED(+)" "REMOVED(-)" "NET"
    printf "%-25s  %8s  %8s  %8s  %8s\n" "-------------------------" "--------" "--------" "----------" "--------"

    # Collect and sort categories
    ALL_CATS=()
    for cat in "${!CATEGORY_MODIFIED[@]}"; do
        ALL_CATS+=("${cat}")
    done
    # Also add categories that only appear in added files
    for cat in "${!CATEGORY_ADDED[@]}"; do
        found=false
        for existing in "${ALL_CATS[@]:-}"; do
            if [ "${existing}" = "${cat}" ]; then
                found=true
                break
            fi
        done
        if ! ${found}; then
            ALL_CATS+=("${cat}")
        fi
    done

    IFS=$'\n' SORTED_CATS=($(sort <<< "$(printf '%s\n' "${ALL_CATS[@]}")"))
    unset IFS

    TOTAL_ADD=0
    TOTAL_REM=0
    TOTAL_MOD=0

    for cat in "${SORTED_CATS[@]}"; do
        mod=${CATEGORY_MODIFIED["${cat}"]:-0}
        add=${CATEGORY_ADDED["${cat}"]:-0}
        rem=${CATEGORY_REMOVED["${cat}"]:-0}
        net=$((add - rem))
        TOTAL_ADD=$((TOTAL_ADD + add))
        TOTAL_REM=$((TOTAL_REM + rem))
        TOTAL_MOD=$((TOTAL_MOD + mod))
        printf "%-25s  %8d  %8d  %10d  %+8d\n" "${cat}" "${mod}" "${add}" "${rem}" "${net}"
    done

    printf "%-25s  %8s  %8s  %8s  %8s\n" "-------------------------" "--------" "--------" "----------" "--------"
    printf "%-25s  %8d  %8d  %10d  %+8d\n" "TOTAL" "${TOTAL_MOD}" "${TOTAL_ADD}" "${TOTAL_REM}" "$((TOTAL_ADD - TOTAL_REM))"
} > "${CATEGORY_FILE}"

# ── Summary ────────────────────────────────────────────────────────────────────
{
    echo "================================================================"
    echo "  Minecraft Server Diff: ${OLD_VERSION} → ${NEW_VERSION}"
    echo "================================================================"
    echo ""
    echo "  Files in ${OLD_VERSION}:  ${OLD_COUNT}"
    echo "  Files in ${NEW_VERSION}:  ${NEW_COUNT}"
    echo ""
    echo "  Added files:      ${ADDED_COUNT}"
    echo "  Removed files:    ${REMOVED_COUNT}"
    echo "  Modified files:   ${MODIFIED_COUNT}"
    echo "  Unchanged files:  ${UNCHANGED_COUNT}"
    echo ""
    echo "================================================================"
    echo ""
    echo "Output files:"
    echo "  summary.txt             — This file"
    echo "  full.diff               — Complete unified diff of all changes"
    echo "  change-stats.txt        — Per-file line change counts"
    echo "  changes-by-category.txt — Changes grouped by MC package"
    echo "  modified-files.txt      — List of modified files"
    echo "  added-files.txt         — List of added files"
    echo "  removed-files.txt       — List of removed files"
} > "${SUMMARY_FILE}"

# ── Print to terminal ─────────────────────────────────────────────────────────
echo ""
echo "  ┌──────────────────────────────────────────────────────┐"
echo "  │  Minecraft Server Diff: ${OLD_VERSION} → ${NEW_VERSION}            │"
echo "  ├──────────────────────────────────────────────────────┤"
printf "  │  Files in %-12s  %5d                         │\n" "${OLD_VERSION}:" "${OLD_COUNT}"
printf "  │  Files in %-12s  %5d                         │\n" "${NEW_VERSION}:" "${NEW_COUNT}"
echo "  │                                                      │"
printf "  │  $(green "Added files:     ")  %5d                         │\n" "${ADDED_COUNT}"
printf "  │  $(red "Removed files:   ")  %5d                         │\n" "${REMOVED_COUNT}"
printf "  │  $(cyan "Modified files:  ")  %5d                         │\n" "${MODIFIED_COUNT}"
printf "  │  Unchanged files:   %5d                         │\n" "${UNCHANGED_COUNT}"
echo "  └──────────────────────────────────────────────────────┘"
echo ""

# Show changes by category
if [ "${MODIFIED_COUNT}" -gt 0 ] || [ "${ADDED_COUNT}" -gt 0 ]; then
    info "Changes by package:"
    echo ""
    cat "${CATEGORY_FILE}"
    echo ""
fi

# Show top modified files by change volume
if [ "${MODIFIED_COUNT}" -gt 0 ]; then
    info "Top 20 most-changed files:"
    echo ""
    sort -t'+' -k2 -rn "${STATS_FILE}" | head -20 | while IFS= read -r line; do
        echo "  ${line}"
    done
    echo ""
fi

# Show added files
if [ "${ADDED_COUNT}" -gt 0 ]; then
    info "Added files:"
    echo ""
    while IFS= read -r f; do
        [ -n "${f}" ] && echo "  $(green "+") ${f}"
    done <<< "${ADDED_FILES}"
    echo ""
fi

# Show removed files
if [ "${REMOVED_COUNT}" -gt 0 ]; then
    info "Removed files:"
    echo ""
    while IFS= read -r f; do
        [ -n "${f}" ] && echo "  $(red "−") ${f}"
    done <<< "${REMOVED_FILES}"
    echo ""
fi

DIFF_SIZE=$(du -h "${FULL_DIFF}" | cut -f1)
ok "Output written to: ${OUTPUT_DIR}/"
ok "Full diff: ${FULL_DIFF} (${DIFF_SIZE})"
echo ""

# Relevance hints for Oxidized
if [ "${MODIFIED_COUNT}" -gt 0 ]; then
    info "Potentially relevant changes for Oxidized:"
    echo ""

    NETWORK_CHANGES=$(grep -c 'network/' "${MODIFIED_LIST}" || true)
    WORLD_CHANGES=$(grep -c 'world/' "${MODIFIED_LIST}" || true)
    SERVER_CHANGES=$(grep -c 'server/' "${MODIFIED_LIST}" || true)
    NBT_CHANGES=$(grep -c 'nbt/' "${MODIFIED_LIST}" || true)
    COMMANDS_CHANGES=$(grep -c 'commands/' "${MODIFIED_LIST}" || true)
    CORE_CHANGES=$(grep -c 'core/' "${MODIFIED_LIST}" || true)

    [ "${NETWORK_CHANGES}" -gt 0 ] && echo "  🔌 Network/Protocol: ${NETWORK_CHANGES} files (oxidized-protocol)"
    [ "${WORLD_CHANGES}" -gt 0 ]   && echo "  🌍 World:            ${WORLD_CHANGES} files (oxidized-world)"
    [ "${SERVER_CHANGES}" -gt 0 ]  && echo "  🖥️  Server:           ${SERVER_CHANGES} files (oxidized-server)"
    [ "${NBT_CHANGES}" -gt 0 ]     && echo "  📦 NBT:              ${NBT_CHANGES} files (oxidized-nbt)"
    [ "${COMMANDS_CHANGES}" -gt 0 ] && echo "  ⌨️  Commands:         ${COMMANDS_CHANGES} files (oxidized-game)"
    [ "${CORE_CHANGES}" -gt 0 ]    && echo "  ⚙️  Core:             ${CORE_CHANGES} files (oxidized-game)"
    echo ""
fi
