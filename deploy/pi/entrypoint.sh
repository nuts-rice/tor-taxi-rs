#!/usr/bin/env bash
#
# Container PID 1: runs Tor and the checker as a single unit.
#
# They are one unit because a checker probing through a dead SOCKS port does
# not fail loudly -- every link simply comes back unreachable, which is
# indistinguishable from a real outage and gets written to D1 as one. So if
# either process exits, this takes the whole container down and lets the
# systemd unit on the host restart the pair.

set -euo pipefail

TORRC="${TORRC:-/etc/tor/torrc}"
READY_FILE="${READY_FILE:-/tmp/tor-bootstrapped}"
BOOTSTRAP_TIMEOUT="${BOOTSTRAP_TIMEOUT:-300}"

tor_pid=""
checker_pid=""

log() { printf '%s entrypoint: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }

# `docker stop` sends TERM. Pass it to both children rather than letting them
# be SIGKILLed when the grace period runs out: Tor flushes state to its
# DataDirectory on a clean shutdown, and that state is the entry guards we
# deliberately persist across restarts.
shutdown() {
  trap - TERM INT
  log "shutting down"
  [[ -n $checker_pid ]] && kill -TERM "$checker_pid" 2>/dev/null || true
  [[ -n $tor_pid ]] && kill -TERM "$tor_pid" 2>/dev/null || true
  wait || true
  exit 0
}
trap shutdown TERM INT

rm -f "$READY_FILE"

# Tor's output goes through a reader that forwards it to the container log and
# watches for the bootstrap line. Process substitution rather than a pipe so
# that $! below is Tor's own PID and not the reader's -- with `tor | while ...`
# it would be the loop, and every liveness check here would be watching the
# wrong process.
tor -f "$TORRC" > >(
  while IFS= read -r line; do
    printf '%s\n' "$line"
    [[ $line == *"Bootstrapped 100%"* ]] && : > "$READY_FILE"
  done
) 2>&1 &
tor_pid=$!
log "tor started (pid $tor_pid)"

# Hold the checker until there is a usable circuit. Without this its opening
# proxy::verify_isolation() call and its entire first sweep run against a Tor
# that is still fetching the consensus, and the sweep lands in D1 with every
# link down -- real-looking bad data, which is the one error this tool must
# not make.
deadline=$(( SECONDS + BOOTSTRAP_TIMEOUT ))
while [[ ! -e $READY_FILE ]]; do
  if ! kill -0 "$tor_pid" 2>/dev/null; then
    log "tor exited during bootstrap"
    exit 1
  fi
  if (( SECONDS >= deadline )); then
    log "tor did not bootstrap within ${BOOTSTRAP_TIMEOUT}s; giving up"
    kill -TERM "$tor_pid" 2>/dev/null || true
    wait || true
    exit 1
  fi
  sleep 2
done
log "tor bootstrapped"

tor-taxi-checker "$@" &
checker_pid=$!
log "checker started (pid $checker_pid)"

# Whichever exits first ends the container.
status=0
wait -n "$tor_pid" "$checker_pid" || status=$?

if ! kill -0 "$tor_pid" 2>/dev/null; then
  # Tor died under a running checker. Always an error, however Tor exited.
  log "tor exited (status $status); stopping the checker"
  kill -TERM "$checker_pid" 2>/dev/null || true
  wait || true
  exit 1
fi

# The checker exited. Propagate its status as-is: under `--once` a clean run
# legitimately ends in 0, and the verification steps depend on seeing that.
log "checker exited (status $status); stopping tor"
kill -TERM "$tor_pid" 2>/dev/null || true
wait || true
exit "$status"
