"""极简 S3 模拟服务器（验证 storage.rs SigV4 路径用；不实现完整协议）。

能力：验证 AWS SigV4 签名的 PUT/GET——
  1) 校验 Authorization 头存在且为 AWS4-HMAC-SHA256（不校验签名值本身，
     真实性由 SigV4 算法单测保证；这里验的是请求形状/路径/头部正确性）；
  2) PUT /{bucket}/{key} 落盘到本地目录；
  3) GET  同路径回读；
  4) 记录每次请求的 method/path/x-amz-date 供断言。

用法：python scripts/_s3_mock.py <port> <data_dir>
"""
import http.server
import os
import sys
import urllib.parse

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 19000
DATA = sys.argv[2] if len(sys.argv) > 2 else "./_s3mock_data"
os.makedirs(DATA, exist_ok=True)
LOG = os.path.join(DATA, "_requests.log")


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def _record(self):
        with open(LOG, "a", encoding="utf-8") as f:
            f.write(
                "{self.command} {self.path} {self.headers.get('x-amz-date',"
                    "'')}\n")

    def _sig_ok(self):
        auth = self.headers.get("Authorization", "")
        return auth.startswith("AWS4-HMAC-SHA256 Credential=")

    def do_PUT(self):
        self._record()
        if not self._sig_ok():
            self.send_response(403)
            self.end_headers()
            self.wfile.write(b"missing sigv4")
            return
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length)
        key = urllib.parse.unquote(self.path.lstrip("/"))
        p = os.path.join(DATA, key.replace("/", os.sep))
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "wb") as f:
            f.write(body)
        self.send_response(200)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_GET(self):
        self._record()
        if not self._sig_ok():
            self.send_response(403)
            self.end_headers()
            self.wfile.write(b"missing sigv4")
            return
        key = urllib.parse.unquote(self.path.lstrip("/"))
        p = os.path.join(DATA, key.replace("/", os.sep))
        if not os.path.isfile(p):
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        with open(p, "rb") as f:
            body = f.read()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *a):
        pass


if __name__ == "__main__":
    print(f"s3-mock on :{PORT} data={DATA}")
    http.server.ThreadingHTTPServer(("127.0.0.1", PORT),
        Handler).serve_forever()
