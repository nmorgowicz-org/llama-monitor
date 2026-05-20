#!/usr/bin/env bash
#
# Manual chat integration test suite
# Validates all chat endpoints against a running llama-monitor instance
# pointed at a live llama-server.
#
# Usage:
#   # Pointing at default monitor (localhost:7778) with its configured session:
#   bash tests/integration/chat-manual.sh
#
#   # Custom monitor URL:
#   bash tests/integration/chat-manual.sh http://127.0.0.1:9999
#
# Prerequisites:
#   - llama-monitor must be running and accessible
#   - An active session must be configured (attach or spawn mode)
#   - The session must point to a reachable llama-server

set -euo pipefail

BASE_URL="${1:-http://127.0.0.1:7778}"
PASS=0
FAIL=0
TOTAL=0

# Fetch the api-token for protected endpoints (used by session endpoints).
API_TOKEN=""
if curl -s "${BASE_URL}/api/internal/api-token" -o /tmp/lm-token.json --connect-timeout 5 2>/dev/null; then
    API_TOKEN=$(cat /tmp/lm-token.json | grep -o '"token":"[^"]*"' | cut -d'"' -f4 || true)
    rm -f /tmp/lm-token.json
fi

# Fetch the db-admin-token for elevated session operations.
DB_ADMIN_TOKEN=""
if curl -s "${BASE_URL}/api/db/admin-token" -o /tmp/lm-db-token.json --connect-timeout 5 2>/dev/null; then
    DB_ADMIN_TOKEN=$(cat /tmp/lm-db-token.json | grep -o '"token":"[^"]*"' | cut -d'"' -f4 || true)
    rm -f /tmp/lm-db-token.json
fi

# ── Helpers ────────────────────────────────────────────────────────────────

assert_http() {
    local label="$1"
    local method="$2"
    local path="$3"
    local expected_code="${4:-200}"
    local body="${5:-}"
    local auth_token="${6:-}"

    TOTAL=$((TOTAL + 1))
    local args=(-s -o /dev/null -w "%{http_code}" -X "$method" "${BASE_URL}${path}" -H "Content-Type: application/json" --connect-timeout 5)

    if [[ -n "$auth_token" ]]; then
        args+=(-H "Authorization: Bearer $auth_token")
    fi

    if [[ -n "$body" ]]; then
        args+=(-d "$body")
    fi

    local code
    code=$(curl "${args[@]}" 2>/dev/null)

    if [[ "$code" == "$expected_code" ]]; then
        PASS=$((PASS + 1))
        echo "  ✅ $label"
    else
        FAIL=$((FAIL + 1))
        echo "  ❌ $label (expected $expected_code, got $code)"
    fi
}

assert_json_contains() {
    local label="$1"
    local method="$2"
    local path="$3"
    local expected="$4"
    local auth_token="${5:-}"

    TOTAL=$((TOTAL + 1))
    local output
    if [[ -n "$auth_token" ]]; then
        output=$(curl -s -X "$method" "${BASE_URL}${path}" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $auth_token" \
            --connect-timeout 5 2>/dev/null || true)
    else
        output=$(curl -s -X "$method" "${BASE_URL}${path}" \
            -H "Content-Type: application/json" \
            --connect-timeout 5 2>/dev/null || true)
    fi

    if echo "$output" | grep -q "$expected"; then
        PASS=$((PASS + 1))
        echo "  ✅ $label"
    else
        FAIL=$((FAIL + 1))
        echo "  ❌ $label (response does not contain '$expected')"
    fi
}

# ── Tests ──────────────────────────────────────────────────────────────────

echo "🧪 Chat Integration Tests"
echo "   Target: $BASE_URL"
echo ""

# Verify monitor is reachable
TOTAL=$((TOTAL + 1))
HEALTH=$(curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/" --connect-timeout 5)
if [[ "$HEALTH" == "200" ]]; then
    PASS=$((PASS + 1))
    echo "  ✅ Monitor is reachable"
else
    FAIL=$((FAIL + 1))
    echo "  ❌ Monitor is not reachable (got $HEALTH). Is it running?"
    echo ""
    echo "Run: cargo run -- --headless"
    exit 1
fi

# ── Setup: Ensure an active session exists ────────────────────────────────

echo ""
echo "── Setup: Checking Active Session ──"

TOTAL=$((TOTAL + 1))
ACTIVE=$(curl -s "${BASE_URL}/api/sessions/active" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_TOKEN" \
    --connect-timeout 5 2>/dev/null || true)

# Check if there's an active session (response should have "ok":true or session data)
if echo "$ACTIVE" | grep -q '"ok"'; then
    PASS=$((PASS + 1))
    SID=$(echo "$ACTIVE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
    echo "  ✅ Active session found: $SID"
else
    # Try to find and activate an attach session
    ALL_SESSIONS=$(curl -s "${BASE_URL}/api/sessions" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $API_TOKEN" \
        --connect-timeout 5 2>/dev/null || true)
    ATTACH_ID=$(echo "$ALL_SESSIONS" | grep -o '"id":"session_[^"]*"' | head -1 | cut -d'"' -f4)
    if [[ -n "$ATTACH_ID" ]]; then
        curl -s -X POST "${BASE_URL}/api/sessions/active" \
            -H "Content-Type: application/json" \
            -H "Authorization: Bearer $API_TOKEN" \
            -d "{\"id\": \"$ATTACH_ID\"}" > /dev/null
        PASS=$((PASS + 1))
        echo "  ✅ Activated session: $ATTACH_ID"
    else
        FAIL=$((FAIL + 1))
        echo "  ❌ No active session. Please configure a session first."
        echo "     Then re-run: bash tests/integration/chat-manual.sh"
        exit 1
    fi
fi

echo ""
echo "── Phase 1: Backend Endpoints ──"

assert_http "GET /api/chat/tabs returns 200" "GET" "/api/chat/tabs"
assert_http "POST /api/chat/abort returns 200" "POST" "/api/chat/abort"

echo ""
echo "── Phase 2: Tab Persistence ──"

# Create a test tab
NOW=$(date +%s%3N)
TAB_JSON="[{
  \"id\": \"test-tab-$$\",
  \"name\": \"Auto Test\",
  \"system_prompt\": \"\",
  \"messages\": [],
  \"model_params\": {
    \"temperature\": 0.8,
    \"top_p\": 0.9,
    \"top_k\": 40,
    \"min_p\": 0.01,
    \"repeat_penalty\": 1.0,
    \"max_tokens\": null
  },
  \"created_at\": $NOW,
  \"updated_at\": $NOW
}]"

assert_http "PUT /api/chat/tabs (create tab)" "PUT" "/api/chat/tabs" 200 "$TAB_JSON"

# Verify persistence
assert_json_contains "Tab persistence verified" "GET" "/api/chat/tabs" "Auto Test"

# Clean up test tab
curl -s -X PUT "${BASE_URL}/api/chat/tabs" \
    -H "Content-Type: application/json" \
    -d '[]' > /dev/null

echo ""
echo "── Phase 3: Chat Completion ──"

# Test: chat endpoint accepts requests and returns valid HTTP
assert_http \
    "POST /api/chat (basic message)" \
    "POST" "/api/chat" 200 \
    '{"messages":[{"role":"user","content":"Say hello in three words."}],"stream":true}'

assert_http \
    "POST /api/chat (with max_tokens)" \
    "POST" "/api/chat" 200 \
    '{"messages":[{"role":"user","content":"Reply with exactly: VALIDATION_SUCCESS"}],"stream":true,"max_tokens":64}'

echo ""
echo "── Phase 4: Model Parameters Passthrough ──"

# Test: restrictive params are accepted (may produce empty response, but should not error)
assert_http \
    "POST /api/chat with all params accepted" \
    "POST" "/api/chat" 200 \
    '{"messages":[{"role":"user","content":"hi"}],"stream":true,"temperature":0.1,"top_p":0.1,"top_k":5,"min_p":0.01,"repeat_penalty":2.0,"max_tokens":8}'

echo ""
echo "── Phase 5: System Prompt Passthrough ──"

# Test: system prompt is accepted (may produce different output, but should not error)
assert_http \
    "POST /api/chat with system prompt accepted" \
    "POST" "/api/chat" 200 \
    '{"messages":[{"role":"system","content":"You are a test assistant."},{"role":"user","content":"hi"}],"stream":true}'

echo ""
echo "── Results ──────────────────────────────────────────────────────────────"
echo ""
echo "  Passed: $PASS / $TOTAL"
echo "  Failed: $FAIL / $TOTAL"
echo ""

if [[ $FAIL -gt 0 ]]; then
    echo "  ⚠️  Some tests failed. Check output above."
    exit 1
else
    echo "  🎉 All tests passed."
    exit 0
fi
