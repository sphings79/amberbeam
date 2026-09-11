#!/bin/sh
# Starts and stops the SFTP server the integration tests run against.
#
#   dev/test-sftp-server.sh start
#   AMBERBEAM_TEST_SFTP=127.0.0.1:2222 cargo test -p amberbeam-core -- --ignored
#   dev/test-sftp-server.sh stop
#
# The password and the key below are test values and deliberately in the open.
# The server holds nothing, listens only on the loopback address, and is thrown
# away afterwards.
set -eu

NAME=amberbeam-test-sftp
IMAGE=linuxserver/openssh-server:latest
PORT=2222
KEYS="$(cd "$(dirname "$0")" && pwd)/sftp-test/keys"

# The directory the listing tests read. Built here rather than by the tests so
# the tests need nothing but a network connection — and so the awkward names
# are in one place where they can be read.
fixture() {
  docker exec "$NAME" sh -c '
    set -e
    rm -rf /config/testdata
    mkdir -p /config/testdata/images
    printf "<!doctype html>" > /config/testdata/index.html
    printf "x" > "/config/testdata/Größe & Maß.txt"
    printf "x" > "/config/testdata/with space.txt"
    printf "x" > /config/testdata/.hidden
    printf "x" > /config/testdata/secret.txt
    chmod 600 /config/testdata/secret.txt
    ln -s /config/testdata/images /config/testdata/current
    ln -s /config/testdata/nowhere /config/testdata/broken
    mkdir -p /config/testdata/locked
    chmod 000 /config/testdata/locked
    chown -R 1000:1000 /config/testdata
  ' >/dev/null
  echo "fixture in /config/testdata"
}

case "${1:-start}" in
  start)
    [ -f "$KEYS/client" ] || {
      mkdir -p "$KEYS"
      ssh-keygen -q -t ed25519 -N "" -C amberbeam-test-client -f "$KEYS/client"
      ssh-keygen -q -t ed25519 -N passwort123 -C amberbeam-test-locked -f "$KEYS/client-locked"
    }
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    docker run -d --name "$NAME" \
      -p "127.0.0.1:$PORT:2222" \
      -e PUID=1000 -e PGID=1000 -e TZ=Etc/UTC \
      -e USER_NAME=amberbeam \
      -e USER_PASSWORD=tannenbaum \
      -e PASSWORD_ACCESS=true \
      -e SUDO_ACCESS=false \
      -e PUBLIC_KEY_FILE=/keys/client.pub \
      -v "$KEYS:/keys:ro" \
      "$IMAGE" >/dev/null
    # Asking SSH itself is the only readiness check that means anything: the
    # log line differs between images, the port is open before sshd answers.
    printf 'waiting for sshd'
    for _ in $(seq 1 60); do
      if ssh-keyscan -p "$PORT" -T 2 127.0.0.1 2>/dev/null | grep -q ssh-ed25519; then
        echo " — ready on 127.0.0.1:$PORT"
        fixture
        exit 0
      fi
      printf .
      sleep 1
    done
    echo " — gave up"
    docker logs "$NAME" 2>&1 | tail -20
    exit 1
    ;;
  stop)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    echo "stopped"
    ;;
  *)
    echo "usage: $0 [start|stop]" >&2
    exit 2
    ;;
esac
