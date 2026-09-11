#!/bin/sh
# Starts and stops the FTP server the integration tests run against.
#
#   dev/test-ftp-server.sh start
#   AMBERBEAM_TEST_FTP=127.0.0.1:2121 AMBERBEAM_TEST_FTP_USER=amberbeam \
#     AMBERBEAM_TEST_FTP_PASSWORD=tannenbaum \
#     cargo test -p amberbeam-core --test ftp -- --test-threads=1
#   dev/test-ftp-server.sh stop
#
# Pure-FTPd, because it speaks the parts the client depends on — MLSD, REST
# STREAM, SIZE, MFMT — and does explicit FTPS with a certificate it signs
# itself, which is exactly the case the certificate dialog exists for.
#
# The image's own command line is replaced rather than extended, to drop its
# -X and -x: those forbid reading and writing files whose name begins with a
# dot. A server may well be set up that way, but a client that cannot be tested
# against .htaccess is not being tested against the thing people came for.
#
# The password below is a test value and deliberately in the open. The server
# holds nothing, listens only on the loopback address, and is thrown away
# afterwards.
set -eu

NAME=amberbeam-test-ftp
IMAGE=stilliard/pure-ftpd:hardened
PORT=2121
# Passive mode needs its own ports, and the client is told this range by the
# server itself. Ten is more than the four simultaneous logins FTP defaults to.
PASV_FROM=30000
PASV_TO=30009

case "${1:-start}" in
  start)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    # The image is built for amd64 only. On an Apple Silicon machine that means
    # emulation, which is slow but works; on the Linux runner it is native.
    docker run -d --name "$NAME" \
      --platform linux/amd64 \
      -p "127.0.0.1:$PORT:21" \
      -p "127.0.0.1:$PASV_FROM-$PASV_TO:$PASV_FROM-$PASV_TO" \
      -e PUBLICHOST=127.0.0.1 \
      -e FTP_USER_NAME=amberbeam \
      -e FTP_USER_PASS=tannenbaum \
      -e FTP_USER_HOME=/home/amberbeam \
      -e "FTP_PASSIVE_PORTS=$PASV_FROM:$PASV_TO" \
      -e ADDED_FLAGS="--tls=1" \
      -e TLS_CN=localhost \
      -e TLS_ORG=AmberBeam \
      -e TLS_C=DE \
      -e TLS_USE_DSAPRAM=true \
      "$IMAGE" \
      /run.sh -l puredb:/etc/pure-ftpd/pureftpd.pdb -E -j -R -P 127.0.0.1 -s -A -Z -H -4 -G \
      >/dev/null

    # Asking the server itself is the only readiness check that means
    # anything: the port is open before pure-ftpd answers on it.
    # Generating the certificate takes a while the first time, and longer
    # under emulation.
    printf 'waiting for pure-ftpd'
    for _ in $(seq 1 60); do
      if printf 'QUIT\r\n' | nc -w 2 127.0.0.1 "$PORT" 2>/dev/null | grep -q '^220'; then
        echo " — ready on 127.0.0.1:$PORT"
        # The directory the listing tests read. Built here rather than by the
        # tests, so the awkward names live in one place where they can be seen.
        docker exec "$NAME" sh -c '
          set -e
          rm -rf /home/amberbeam/testdata
          mkdir -p /home/amberbeam/testdata/images
          printf "<!doctype html>" > /home/amberbeam/testdata/index.html
          printf "x" > "/home/amberbeam/testdata/Größe & Maß.txt"
          printf "x" > "/home/amberbeam/testdata/with space.txt"
          printf "x" > /home/amberbeam/testdata/.hidden
          chown -R ftpuser:ftpgroup /home/amberbeam/testdata
        ' >/dev/null
        echo "fixture in /testdata"
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
