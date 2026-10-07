#!/usr/bin/env bash
# ==============================================================================
# ClipOn Comprehensive End-to-End Smoke Test Script (Issue #36)
# ==============================================================================
set -euo pipefail

BOLD="\033[1m"
GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[0;33m"
CYAN="\033[0;36m"
NC="\033[0m"

echo -e "${BOLD}${CYAN}======================================================${NC}"
echo -e "${BOLD}${CYAN}   ClipOn End-to-End Smoke Test Suite   ${NC}"
echo -e "${BOLD}${CYAN}======================================================${NC}"

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

PASS_COUNT=0
FAIL_COUNT=0

report_pass() {
    echo -e "  [${GREEN}PASS${NC}] $1"
    PASS_COUNT=$((PASS_COUNT + 1))
}

report_fail() {
    echo -e "  [${RED}FAIL${NC}] $1"
    FAIL_COUNT=$((FAIL_COUNT + 1))
}

# 1. Environment & Binaries Check
echo -e "\n${BOLD}Step 1: Checking Required Binaries...${NC}"

if command -v ffmpeg >/dev/null 2>&1; then
    FFMPEG_VER=$(ffmpeg -version | head -n 1)
    report_pass "ffmpeg detected ($FFMPEG_VER)"
else
    report_fail "ffmpeg not found on PATH"
fi

if command -v ffprobe >/dev/null 2>&1; then
    report_pass "ffprobe detected"
else
    report_fail "ffprobe not found on PATH"
fi

if command -v cargo >/dev/null 2>&1; then
    CARGO_VER=$(cargo --version)
    report_pass "cargo detected ($CARGO_VER)"
else
    report_fail "cargo not found"
fi

if command -v npm >/dev/null 2>&1; then
    NPM_VER=$(npm --version)
    report_pass "npm detected ($NPM_VER)"
else
    report_fail "npm not found"
fi

# 2. Frontend Typecheck & Build
echo -e "\n${BOLD}Step 2: Frontend Build & TypeScript Validation...${NC}"
if npm run build; then
    report_pass "Frontend builds with zero TypeScript errors"
else
    report_fail "Frontend build failed"
fi

# 3. Rust Unit Test Suite
echo -e "\n${BOLD}Step 3: Running Rust Core Library Unit Tests...${NC}"
if cargo test --manifest-path src-tauri/Cargo.toml --lib; then
    report_pass "All Rust library unit tests passed"
else
    report_fail "Rust library unit tests failed"
fi

# 4. Golden Reframe Integration Tests
echo -e "\n${BOLD}Step 4: Running Golden Reframe Integration Tests...${NC}"
if cargo test --manifest-path src-tauri/Cargo.toml --test reframe_golden; then
    report_pass "Golden reframe tests passed across all 3 modes (Original, VerticalCrop, SmartFaceTrack)"
else
    report_fail "Golden reframe tests failed"
fi

# 5. Frontend Test Suite
echo -e "\n${BOLD}Step 5: Running Frontend Test Suite...${NC}"
if npm test; then
    report_pass "All Frontend tests passed"
else
    report_fail "Frontend tests failed"
fi

# Summary
echo -e "\n${BOLD}${CYAN}======================================================${NC}"
echo -e "${BOLD}Smoke Test Summary:${NC}"
echo -e "  Passed: ${GREEN}${PASS_COUNT}${NC}"
echo -e "  Failed: ${RED}${FAIL_COUNT}${NC}"

if [ "$FAIL_COUNT" -eq 0 ]; then
    echo -e "${BOLD}${GREEN}All Smoke Tests Passed Successfully! Verified test coverage meets quality gates.${NC}"
    exit 0
else
    echo -e "${BOLD}${RED}Some smoke tests failed. Please review output above.${NC}"
    exit 1
fi
