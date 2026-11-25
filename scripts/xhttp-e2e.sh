#!/usr/bin/env bash
# End-to-end check for the ProtoGrid sing-box XHTTP fork.
#
# It builds the fork, starts Xray with an XHTTP inbound and checks TCP and UDP
# through the fork's local mixed inbound.
#
# Requirements: bash, git, go, python3, curl, openssl.
# The Xray binary is downloaded on first run.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORK="$ROOT/forks/sing-box"
WORK="${WORK:-/tmp/opencode/protogrid-xhttp-e2e}"
GO="${GO:-go}"
MODE="${MODE:-stream-one}"
SKIP_UDP="${SKIP_UDP:-0}"
KEEP="${KEEP:-0}"

if ! command -v "$GO" >/dev/null 2>&1; then
  for candidate in /tmp/opencode/go127/bin/go /tmp/opencode/go/bin/go; do
    if [ -x "$candidate" ]; then
      GO="$candidate"
      break
    fi
  done
fi

if [ ! -d "$FORK" ]; then
  echo "fork not found: $FORK" >&2
  exit 1
fi

mkdir -p "$WORK"
cd "$WORK"

cleanup() {
  for pid_file in singbox.pid xray.pid http.pid; do
    if [ -f "$pid_file" ]; then
      kill "$(cat "$pid_file")" 2>/dev/null || true
    fi
  done
  if [ "$KEEP" != "1" ]; then
    rm -rf "$WORK/www" "$WORK"/*.pid
  fi
}
trap cleanup EXIT

echo "== build fork =="
(
  cd "$FORK"
  "$GO" build -tags "with_quic,with_utls,with_clash_api,with_gvisor" \
    -ldflags "-X github.com/sagernet/sing-box/constant.Version=1.14.4-protogrid.1" \
    -o "$WORK/sing-box" ./cmd/sing-box
)

if [ ! -x "$WORK/xray" ]; then
  echo "== download Xray =="
  curl -sS https://api.github.com/repos/XTLS/Xray-core/releases/latest \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); print(next(a["browser_download_url"] for a in d["assets"] if "linux-64" in a["name"] and a["name"].endswith(".zip")))' \
    > xray.url
  curl -sSL -o xray.zip "$(cat xray.url)"
  python3 -c 'import zipfile; zipfile.ZipFile("xray.zip").extractall(".")'
  chmod +x xray
fi

if [ ! -f cert.pem ] || [ ! -f key.pem ]; then
  openssl req -x509 -newkey rsa:2048 -nodes -keyout key.pem -out cert.pem \
    -days 2 -subj "/CN=localhost" \
    -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" >/dev/null 2>&1
fi

cat > server.json <<'JSON'
{
  "log": { "loglevel": "info" },
  "inbounds": [
    {
      "listen": "127.0.0.1",
      "port": 18443,
      "protocol": "vless",
      "settings": {
        "clients": [{ "id": "b831381d-6324-4d53-ad4f-8cda48b30811" }],
        "decryption": "none"
      },
      "streamSettings": {
        "network": "xhttp",
        "security": "tls",
        "tlsSettings": {
          "alpn": ["h2"],
          "certificates": [
            { "certificateFile": "cert.pem", "keyFile": "key.pem" }
          ]
        },
        "xhttpSettings": { "path": "/x", "mode": "auto" }
      }
    }
  ],
  "outbounds": [{ "protocol": "freedom", "tag": "freedom" }]
}
JSON

cat > client.json <<JSON
{
  "log": { "level": "info" },
  "inbounds": [
    { "type": "mixed", "listen": "127.0.0.1", "listen_port": 12080 }
  ],
  "outbounds": [
    {
      "type": "vless",
      "tag": "proxy",
      "server": "127.0.0.1",
      "server_port": 18443,
      "uuid": "b831381d-6324-4d53-ad4f-8cda48b30811",
      "tls": { "enabled": true, "server_name": "localhost", "insecure": true },
      "transport": {
        "type": "xhttp",
        "host": "localhost",
        "path": "/x",
        "mode": "$MODE"
      }
    }
  ]
}
JSON

mkdir -p www
echo "hello-through-xhttp" > www/index.txt

nohup ./xray run -c server.json > xray.log 2>&1 &
echo $! > xray.pid
nohup python3 -m http.server 18000 --bind 127.0.0.1 --directory www > http.log 2>&1 &
echo $! > http.pid
nohup ./sing-box run -c client.json > sing-box.log 2>&1 &
echo $! > singbox.pid

for _ in $(seq 1 50); do
  if curl -sS --max-time 1 -x http://127.0.0.1:12080 http://127.0.0.1:18000/index.txt >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

echo "== TCP ($MODE) =="
body="$(curl -sS --max-time 10 -x http://127.0.0.1:12080 http://127.0.0.1:18000/index.txt)"
test "$body" = "hello-through-xhttp"
echo "TCP OK"

if [ "$SKIP_UDP" != "1" ]; then
  echo "== UDP ($MODE) =="
  python3 - <<'PY'
import socket, struct
tcp = socket.create_connection(("127.0.0.1", 12080), timeout=5)
tcp.sendall(b"\x05\x01\x00")
assert tcp.recv(2) == b"\x05\x00"
tcp.sendall(b"\x05\x03\x00\x01\x00\x00\x00\x00\x00\x00")
resp = tcp.recv(10)
assert resp[:2] == b"\x05\x00", resp
relay = (socket.inet_ntoa(resp[4:8]), struct.unpack("!H", resp[8:10])[0])
udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
udp.settimeout(8)
query = b"\xab\xcd\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00"
query += b"\x07example\x03com\x00\x00\x01\x00\x01"
packet = b"\x00\x00\x00\x01" + socket.inet_aton("8.8.8.8") + struct.pack("!H", 53) + query
udp.sendto(packet, relay)
data, _ = udp.recvfrom(2048)
assert len(data) > 12, data
print("UDP OK")
PY
fi

echo "All XHTTP checks passed ($MODE)"
