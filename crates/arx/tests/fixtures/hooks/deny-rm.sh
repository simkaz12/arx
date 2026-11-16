#!/bin/sh
grep -q 'rm ' && printf '%s' '{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"rm is blocked"}}' || true
