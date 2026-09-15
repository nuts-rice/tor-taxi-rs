#!/usr/bin/env bash
#
# Proves the plumbing is up -- NOT that sweeps are landing. Nothing in the
# checker records when the last sweep succeeded, so this cannot tell a healthy
# service from one whose D1 writes have failed for an hour. The journal can.

set -uo pipefail

READY_FILE="${READY_FILE:-/tmp/tor-bootstrapped}"
SOCKS_HOST="${SOCKS_HOST:-127.0.0.1}"
SOCKS_PORT="${SOCKS_PORT:-9050}"

fail() { printf 'unhealthy: %s\n' "$1" >&2; exit 1; }

[[ -e $READY_FILE ]] || fail "tor has not reported Bootstrapped 100%"

# bash's own /dev/tcp: the slim image ships neither nc nor curl.
timeout 5 bash -c "exec 3<>/dev/tcp/${SOCKS_HOST}/${SOCKS_PORT}" 2>/dev/null \
  || fail "tor's SOCKS port ${SOCKS_HOST}:${SOCKS_PORT} is refusing connections"

# /proc rather than pgrep, which lives in procps and is not in the slim image.
# $1 is unquoted so it acts as a glob, and the caller needs one: /proc/*/comm
# truncates at 15 chars, so "tor-taxi-checker" reads back as "tor-taxi-checke".
running() {
  local comm
  for comm in /proc/[0-9]*/comm; do
    [[ -r $comm ]] || continue
    [[ $(< "$comm") == $1 ]] && return 0
  done
  return 1
}

running 'tor'              || fail "no tor process"
running 'tor-taxi-checke*' || fail "no checker process"

echo ok
