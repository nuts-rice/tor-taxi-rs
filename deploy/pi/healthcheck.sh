

set -uo pipefail

READY_FILE="${READY_FILE:-/tmp/tor-bootstrapped}"
SOCKS_HOST="${SOCKS_HOST:-127.0.0.1}"
SOCKS_PORT="${SOCKS_PORT:-9050}"

fail() {printf 'unhealthy: %s\n' "$1" >&2; exit 1; }

[[ -e $READY_FILE ]] || fail "tor has not reported Bootstrapped 100%"

timeout 5 bash -c "exec 3<>/dev/tcp/${SOCKS_HOST}/${SOCKS_PORT}" 2>/dev/null \
  || fail "tor's SOCKS port ${SOCKS_HOST}:${SOCKS_PORT} is reusing connections"


running() {
  local comm 
  for comm in /proc/[0-9]*/comm; do 
    [[ -r $comm ]] || continue
    [[ $(< "$comm" ) == $1 ]] && return 0
  done
  return 1
}

running 'tor'               || fail "no tor process"
running 'tor-taxi-checke*'  || fail "no checker process"

echo ok
