#!/usr/bin/env python3
"""批量替换 qB/TR 种子 Tracker 域名（默认 dry-run）。

只换 host 或显式给出的 scheme/port，路径与 query（passkey）
原样保留。一条命令：

  python scripts/tracker_domain_migrate.py qb \
      --url http://127.0.0.1:8080 --old tr.old.com \
      --new tr.new.com --user admin --password xxx [--apply]

tr 同参；退出码 0/1/2/3/4 = 成功/单项失败/参数/认证/网络。
"""
import argparse
import base64
import json
import re
import sys
from collections import namedtuple
from http.cookies import CookieError, SimpleCookie
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode, urlsplit, urlunsplit
from urllib.request import Request, urlopen

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.stderr.reconfigure(encoding="utf-8", errors="replace")

EXIT_ITEM_FAILED, EXIT_USAGE, EXIT_AUTH, EXIT_NETWORK = 1, 2, 3, 4
SENSITIVE_PARAMS = {"passkey", "token", "authkey", "torrent_pass",
                    "apikey", "api_key"}
# 凭据形状含下划线/连字符（fxr_/base64url）；口径同服务端 log_safe_path
LONG_ALNUM = re.compile(r"[A-Za-z0-9_-]{32,}\Z")
BARE_TOKEN = re.compile(r"[A-Za-z0-9_-]{32,}")
Endpoint = namedtuple("Endpoint", "scheme host port")
Response = namedtuple("Response", "status headers body")

class MigrateError(Exception):
    exit_code = EXIT_ITEM_FAILED

class AuthError(MigrateError):
    exit_code = EXIT_AUTH

class NetworkError(MigrateError):
    exit_code = EXIT_NETWORK

class ProtocolError(NetworkError):
    pass

class TrackerUrlError(ValueError):
    pass

def _header(headers, name):
    return next((value for key, value in headers.items()
                 if key.lower() == name.lower()), None)

def _safe_text(text, limit=80):
    # 服务端报错可能回显 URL/密钥，先通杀长字母数字串再截断
    text = BARE_TOKEN.sub("***", " ".join(str(text).split()))
    return text if len(text) <= limit else text[:limit] + "..."

def parse_endpoint(spec):
    """裸 host / host:port / 完整 origin -> Endpoint。"""
    text = (spec or "").strip()
    if not text:
        raise TrackerUrlError("空 host")
    if "://" in text:
        parts = urlsplit(text)
        scheme = parts.scheme.lower()
        if not scheme:
            raise TrackerUrlError("origin 缺少 scheme")
        if parts.path not in ("", "/") or parts.query or parts.fragment:
            raise TrackerUrlError("origin 不能带路径/参数")
        if parts.username is not None or parts.password is not None:
            raise TrackerUrlError("origin 不能带用户信息")
    else:
        scheme = None
        if any(ch in text for ch in "/?#"):
            raise TrackerUrlError("裸 host 不能带路径/参数")
        parts = urlsplit("//" + text)
    if not parts.hostname:
        raise TrackerUrlError("缺少 host")
    try:
        port = parts.port
    except ValueError as exc:
        raise TrackerUrlError("端口不合法") from exc
    return Endpoint(scheme, parts.hostname.lower(), port)

def _url_port(parts):
    try:
        return parts.port
    except ValueError as exc:
        raise TrackerUrlError("URL 端口不合法") from exc

def transform_tracker_url(raw, old, new):
    """返回 (新 URL, 是否变化)；路径/query/fragment 不重编码。"""
    parts = urlsplit(raw)
    if not parts.scheme or not parts.netloc or not parts.hostname:
        raise TrackerUrlError("不是绝对 URL")
    if parts.hostname.lower() != old.host:
        return raw, False
    if old.scheme and parts.scheme.lower() != old.scheme:
        return raw, False
    raw_port = _url_port(parts)
    if old.port is not None and raw_port != old.port:
        return raw, False
    scheme = new.scheme or parts.scheme
    port = new.port if new.port is not None else raw_port
    host = new.host
    if ":" in host:
        host = "[%s]" % host
    userinfo = ""
    if "@" in parts.netloc:
        userinfo = parts.netloc.rsplit("@", 1)[0] + "@"
    netloc = userinfo + host
    if port is not None:
        netloc += ":%d" % port
    out = urlunsplit((scheme, netloc, parts.path, parts.query,
                      parts.fragment))
    return out, out != raw

def _mask_segment(segment):
    return "***" if LONG_ALNUM.match(segment) else segment

def _mask_query(query):
    pairs = []
    for pair in query.split("&"):
        name, _sep, _value = pair.partition("=")
        key = name.lower()
        if key in SENSITIVE_PARAMS or key.endswith(
                ("passkey", "token")):
            pairs.append(name + "=***")
        else:
            pairs.append(pair)
    return "&".join(pairs)

def mask_url(url):
    """展示用：长字母数字路径段与 passkey/token 等参数值变 ***。"""
    try:
        parts = urlsplit(url)
    except ValueError:
        return "***"
    if not parts.scheme or not parts.netloc:
        return "/".join(_mask_segment(s) for s in url.split("/"))
    netloc = parts.netloc
    if "@" in netloc:
        netloc = "***@" + netloc.rsplit("@", 1)[1]
    path = "/".join(_mask_segment(s) for s in parts.path.split("/"))
    return urlunsplit((parts.scheme, netloc, path,
                       _mask_query(parts.query),
                       _mask_segment(parts.fragment)))

def _http_parts(url):
    parts = urlsplit(url)
    if parts.scheme not in ("http", "https") or not parts.hostname:
        raise TrackerUrlError("--url 需为 http(s) 地址")
    if parts.query or parts.fragment:
        raise TrackerUrlError("--url 不能带 query/fragment")
    return parts

def transmission_rpc_url(url):
    parts = _http_parts(url)
    path = parts.path.rstrip("/")
    if path.endswith("/transmission"):
        path += "/rpc"
    elif not path.endswith("/transmission/rpc"):
        path += "/transmission/rpc"
    return urlunsplit((parts.scheme, parts.netloc, path, "", ""))

class UrllibTransport:
    def __init__(self, timeout=20.0):
        self.timeout = timeout

    def request(self, method, url, data=None, headers=None):
        req = Request(url, data=data, headers=headers or {},
                      method=method)
        try:
            with urlopen(req, timeout=self.timeout) as resp:
                return Response(getattr(resp, "status", 200),
                                resp.headers, resp.read())
        except HTTPError as exc:
            # 409 协商与 4xx 判定都要看状态码，不在这里抛
            return Response(exc.code, exc.headers, exc.read())
        except (URLError, OSError) as exc:
            reason = getattr(exc, "reason", exc)
            raise NetworkError("连接失败: %s" % type(reason).__name__)
        except Exception as exc:
            raise NetworkError("请求失败: %s" % type(exc).__name__)

class QbClient:
    def __init__(self, transport, base_url, username=None,
                 password=None, api_key=None):
        self.transport = transport
        self.base = _http_parts(base_url).geturl().rstrip("/")
        self.username = username
        self.password = password
        self.api_key = api_key
        self.cookies = {}

    def _headers(self, extra=None):
        headers = {"Referer": self.base + "/"}
        if self.api_key:
            headers["Authorization"] = "Bearer " + self.api_key
        if self.cookies:
            headers["Cookie"] = "; ".join(
                "%s=%s" % item for item in self.cookies.items())
        headers.update(extra or {})
        return headers

    def _request(self, method, path, data=None, headers=None):
        resp = self.transport.request(
            method, self.base + path, data=data,
            headers=self._headers(headers))
        if resp.status in (401, 403):
            raise AuthError("qB 认证失败（HTTP %d）" % resp.status)
        return resp

    def _take_cookies(self, resp):
        for key, value in resp.headers.items():
            if key.lower() != "set-cookie":
                continue
            jar = SimpleCookie()
            try:
                jar.load(value)
            except CookieError:
                continue
            for name, morsel in jar.items():
                self.cookies[name] = morsel.value

    def login(self):
        if self.api_key:
            return  # qB >=5.2；API key 模式不建会话
        body = urlencode({
            "username": self.username or "",
            "password": self.password or ""}).encode("ascii")
        resp = self._request(
            "POST", "/api/v2/auth/login", body,
            {"Content-Type": "application/x-www-form-urlencoded"})
        if resp.status != 200:
            raise AuthError("qB 登录失败（HTTP %d）" % resp.status)
        text = resp.body.decode("utf-8", "replace").strip()
        if text != "Ok.":
            raise AuthError("qB 登录被拒绝（账号/密码错误或 IP 被封）")
        self._take_cookies(resp)

    def _json(self, method, path, what):
        resp = self._request(method, path)
        if resp.status != 200:
            raise ProtocolError("%s失败（HTTP %d）" % (what, resp.status))
        try:
            return json.loads(resp.body)
        except ValueError as exc:
            raise ProtocolError("%s返回非 JSON" % what) from exc

    def torrents(self):
        data = self._json("GET", "/api/v2/torrents/info", "qB 枚举")
        if not isinstance(data, list):
            raise ProtocolError("qB 枚举返回结构异常")
        return data

    def trackers(self, torrent_hash):
        path = "/api/v2/torrents/trackers?" + urlencode(
            {"hash": torrent_hash})
        data = self._json("GET", path, "qB 读取 tracker")
        if not isinstance(data, list):
            raise ProtocolError("qB tracker 返回结构异常")
        return data

    def edit_tracker(self, torrent_hash, old_url, new_url):
        body = urlencode({
            "hash": torrent_hash, "origUrl": old_url,
            "newUrl": new_url}).encode("utf-8")
        resp = self._request(
            "POST", "/api/v2/torrents/editTracker", body,
            {"Content-Type": "application/x-www-form-urlencoded"})
        if resp.status != 200:
            raise ProtocolError(
                "qB editTracker 失败（HTTP %d）" % resp.status)

class TransmissionClient:
    def __init__(self, transport, url, username=None, password=None):
        self.transport = transport
        self.url = transmission_rpc_url(url)
        self.username = username
        self.password = password
        self.session_id = None

    def _headers(self):
        headers = {"Content-Type": "application/json"}
        if self.username is not None:
            token = ("%s:%s" % (self.username, self.password or "")
                     ).encode("utf-8")
            encoded = base64.b64encode(token).decode("ascii")
            headers["Authorization"] = "Basic " + encoded
        if self.session_id:
            headers["X-Transmission-Session-Id"] = self.session_id
        return headers

    def rpc(self, method, arguments):
        payload = json.dumps({"method": method,
                              "arguments": arguments}).encode("utf-8")
        for attempt in (0, 1):
            resp = self.transport.request(
                "POST", self.url, payload, self._headers())
            if resp.status == 409:
                sid = _header(resp.headers,
                              "X-Transmission-Session-Id")
                if not sid or attempt:
                    raise ProtocolError("TR 会话协商失败")
                self.session_id = sid
                continue
            if resp.status in (401, 403):
                raise AuthError("TR 认证失败（HTTP %d）" % resp.status)
            if resp.status != 200:
                raise ProtocolError(
                    "TR RPC 失败（HTTP %d）" % resp.status)
            try:
                data = json.loads(resp.body)
            except ValueError as exc:
                raise ProtocolError("TR RPC 返回非 JSON") from exc
            if not isinstance(data, dict):
                raise ProtocolError("TR RPC 返回结构异常")
            if data.get("result") != "success":
                raise ProtocolError("TR RPC 返回失败: %s" % _safe_text(
                    data.get("result")))
            result = data.get("arguments")
            return result if isinstance(result, dict) else {}
        raise ProtocolError("TR 会话协商失败")

class Stats:
    def __init__(self):
        self.torrents = self.scanned = self.need = 0
        self.success = self.skipped = self.failed = 0

    def line(self, apply):
        mode = "apply" if apply else "dry-run"
        return ("[摘要] 种子=%d 扫描=%d 需改=%d 成功=%d "
                "跳过=%d 失败=%d (%s)" % (
                    self.torrents, self.scanned, self.need,
                    self.success, self.skipped, self.failed, mode))

def _short(text, limit=36):
    text = " ".join(str(text).split())
    return text if len(text) <= limit else text[:limit] + "..."

def _plan(raw, old, new):
    try:
        new_url, changed = transform_tracker_url(raw, old, new)
    except ValueError:
        return None
    return ((new_url, mask_url(raw), mask_url(new_url))
            if changed else None)

def _qb_units(client, stats):
    for torrent in client.torrents():
        stats.torrents += 1
        torrent_hash = torrent.get("hash") or ""
        name = _short(torrent.get("name") or torrent_hash)
        try:
            trackers = client.trackers(torrent_hash)
        except MigrateError as exc:
            if isinstance(exc, AuthError):
                raise
            stats.failed += 1
            print("[失败] %s 读取 tracker: %s" % (name, exc))
            continue
        yield name, torrent_hash, [
            (t.get("url") or "", None) for t in trackers]

def _tr_units(client, stats):
    data = client.rpc("torrent-get", {
        "fields": ["id", "hashString", "trackers"],
    })
    for torrent in data.get("torrents") or []:
        stats.torrents += 1
        name = _short(torrent.get("hashString") or torrent.get("id"))
        yield name, torrent.get("id"), [
            (t.get("announce") or "", t.get("id"))
            for t in torrent.get("trackers") or []]

def _run_migration(units, apply_one, old, new, apply, stats):
    for name, key, trackers in units:
        for raw, ref in trackers:
            stats.scanned += 1
            item = _plan(raw, old, new)
            if item is None:
                stats.skipped += 1
                continue
            new_url, old_masked, new_masked = item
            stats.need += 1
            if not apply:
                print("[需改] %s %s -> %s" % (
                    name, old_masked, new_masked))
                continue
            try:
                apply_one(key, ref, raw, new_url)
            except AuthError:
                raise
            except MigrateError as exc:
                stats.failed += 1
                print("[失败] %s %s -> %s: %s" % (
                    name, old_masked, new_masked, exc))
                continue
            stats.success += 1
            print("[成功] %s %s -> %s" % (
                name, old_masked, new_masked))

def run_qb(client, old, new, apply):
    stats = Stats()
    _run_migration(
        _qb_units(client, stats),
        lambda hash_, _ref, raw, url: client.edit_tracker(
            hash_, raw, url),
        old, new, apply, stats)
    return stats

def run_tr(client, old, new, apply):
    stats = Stats()
    def apply_one(torrent_id, tracker_id, _raw, new_url):
        client.rpc("torrent-set", {
            "ids": [torrent_id],
            "trackerReplace": [[tracker_id, new_url]]})

    _run_migration(_tr_units(client, stats), apply_one,
                   old, new, apply, stats)
    return stats

def build_parser():
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--url", required=True,
                        help="qB/TR WebUI 根地址")
    common.add_argument("--old", required=True,
                        help="旧 host、host:port 或完整 origin")
    common.add_argument("--new", required=True,
                        help="新 host、host:port 或完整 origin")
    common.add_argument("--user", default=None, help="登录用户名")
    common.add_argument("--password", default=None, help="登录密码")
    common.add_argument("--apply", action="store_true",
                        help="真正写回；缺省只 dry-run")
    common.add_argument("--timeout", type=float, default=20.0,
                        help="HTTP 超时秒数（默认 20）")
    parser = argparse.ArgumentParser(
        description="批量替换 qB/TR 种子 Tracker 域名；默认 dry-run。",
        epilog=(
            "示例（先 dry-run 看清单，确认后加 --apply）：\n"
            "  python scripts/tracker_domain_migrate.py qb \\\n"
            "      --url http://127.0.0.1:8080 --old tr.old.com \\\n"
            "      --new tr.new.com --user admin --password xxx\n"
            "tr 同参（--url 给 Web 根地址，自动补 /transmission/rpc）。\n"
            "--old/--new 支持裸 host、host:port、完整 origin。\n"
            "退出码：0 成功；1 单项失败；2 参数；3 认证；4 网络/协议。"),
        formatter_class=argparse.RawDescriptionHelpFormatter)
    subs = parser.add_subparsers(dest="client", required=True)
    qb = subs.add_parser("qb", parents=[common], help="qBittorrent")
    qb.add_argument(
        "--api-key", default=None,
        help="qB >=5.2 的 API key，优先于 --user/--password")
    subs.add_parser("tr", parents=[common], help="Transmission")
    return parser

def main(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        old = parse_endpoint(args.old)
        new = parse_endpoint(args.new)
        base = _http_parts(args.url).geturl().rstrip("/")
        rpc_url = (transmission_rpc_url(args.url)
                   if args.client == "tr" else None)
    except ValueError as exc:
        print("[错误] 参数无效: %s" % exc, file=sys.stderr)
        return EXIT_USAGE
    if args.timeout <= 0:
        print("[错误] --timeout 必须大于 0", file=sys.stderr)
        return EXIT_USAGE
    transport = UrllibTransport(args.timeout)
    stats = Stats()
    try:
        if args.client == "qb":
            client = QbClient(transport, base, args.user,
                              args.password, args.api_key)
            client.login()
            stats = run_qb(client, old, new, args.apply)
        else:
            client = TransmissionClient(
                transport, rpc_url, args.user, args.password)
            stats = run_tr(client, old, new, args.apply)
    except MigrateError as exc:
        print("[错误] %s" % exc, file=sys.stderr)
        print(stats.line(args.apply), file=sys.stderr)
        return exc.exit_code
    print(stats.line(args.apply))
    if not args.apply:
        print("dry-run: 未写回客户端；确认后加 --apply")
    return EXIT_ITEM_FAILED if stats.failed else 0

if __name__ == "__main__":
    sys.exit(main())
