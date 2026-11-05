#!/usr/bin/env python3
"""tracker_domain_migrate 离线单测；不连接真 qB/TR 服务。"""
import base64
import io
import json
import os
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest import mock
from urllib.parse import parse_qs

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from scripts import tracker_domain_migrate as mod  # noqa: E402
from scripts.tracker_domain_migrate import (  # noqa: E402
    AuthError,
    NetworkError,
    ProtocolError,
    QbClient,
    Response,
    TrackerUrlError,
    TransmissionClient,
    main,
    mask_url,
    parse_endpoint,
    run_qb,
    run_tr,
    transform_tracker_url,
    transmission_rpc_url,
)

KEY = "0123456789abcdef0123456789abcdef"
SESSION_HEADER = "X-Transmission-Session-Id"


class FakeTransport:
    def __init__(self, responses):
        self.responses = list(responses)
        self.calls = []

    def request(self, method, url, data=None, headers=None):
        self.calls.append({
            "method": method,
            "url": url,
            "data": data,
            "headers": dict(headers or {}),
        })
        if not self.responses:
            raise AssertionError("unexpected request")
        item = self.responses.pop(0)
        if isinstance(item, Exception):
            raise item
        return item


def form(call):
    return parse_qs(call["data"].decode("utf-8"))


class TransformTests(unittest.TestCase):
    def test_bare_host_keeps_path_query_fragment(self):
        raw = ("https://tr.old.com/announce/%s"
               "?passkey=%s&x=1#frag" % (KEY, KEY))
        out, changed = transform_tracker_url(
            raw, parse_endpoint("tr.old.com"),
            parse_endpoint("tr.new.com"))
        self.assertTrue(changed)
        self.assertEqual(
            ("https://tr.new.com/announce/%s"
             "?passkey=%s&x=1#frag" % (KEY, KEY)), out)

    def test_old_host_mismatch_untouched(self):
        raw = "https://other.com/announce/%s" % KEY
        out, changed = transform_tracker_url(
            raw, parse_endpoint("tr.old.com"),
            parse_endpoint("tr.new.com"))
        self.assertFalse(changed)
        self.assertEqual(raw, out)

    def test_old_port_kept_when_new_has_no_port(self):
        raw = "http://tr.old.com:8080/announce/k?x=1"
        out, changed = transform_tracker_url(
            raw, parse_endpoint("tr.old.com:8080"),
            parse_endpoint("tr.new.com"))
        self.assertTrue(changed)
        self.assertEqual(
            "http://tr.new.com:8080/announce/k?x=1", out)

    def test_old_port_mismatch_untouched(self):
        raw = "http://tr.old.com:8080/announce/k"
        out, changed = transform_tracker_url(
            raw, parse_endpoint("tr.old.com:8081"),
            parse_endpoint("tr.new.com"))
        self.assertFalse(changed)
        self.assertEqual(raw, out)

    def test_new_port_overrides(self):
        raw = "http://tr.old.com:8080/announce/k"
        out, _ = transform_tracker_url(
            raw, parse_endpoint("tr.old.com"),
            parse_endpoint("tr.new.com:9090"))
        self.assertEqual("http://tr.new.com:9090/announce/k", out)

    def test_full_origin_changes_scheme_and_port(self):
        raw = "https://tr.old.com:443/announce/%s?passkey=%s" % (
            KEY, KEY)
        out, _ = transform_tracker_url(
            raw, parse_endpoint("https://tr.old.com:443"),
            parse_endpoint("http://tr.new.com:8080"))
        self.assertEqual(
            "http://tr.new.com:8080/announce/%s?passkey=%s" % (
                KEY, KEY), out)

    def test_old_origin_scheme_must_match(self):
        raw = "https://tr.old.com/announce/k"
        out, changed = transform_tracker_url(
            raw, parse_endpoint("http://tr.old.com"),
            parse_endpoint("tr.new.com"))
        self.assertFalse(changed)
        self.assertEqual(raw, out)

    def test_new_origin_without_port_keeps_url_port(self):
        raw = "https://tr.old.com:8443/announce/k"
        out, _ = transform_tracker_url(
            raw, parse_endpoint("tr.old.com"),
            parse_endpoint("http://tr.new.com"))
        self.assertEqual("http://tr.new.com:8443/announce/k", out)

    def test_percent_encoding_preserved(self):
        raw = ("https://tr.old.com/announce/a%2Fb"
               "?passkey=abc%2Fdef&x=%E4%B8%AD#f%20g")
        out, _ = transform_tracker_url(
            raw, parse_endpoint("tr.old.com"),
            parse_endpoint("tr.new.com"))
        self.assertEqual(
            "https://tr.new.com/announce/a%2Fb"
            "?passkey=abc%2Fdef&x=%E4%B8%AD#f%20g", out)

    def test_already_new_host_not_changed(self):
        raw = "https://tr.new.com/announce/k"
        out, changed = transform_tracker_url(
            raw, parse_endpoint("tr.old.com"),
            parse_endpoint("tr.new.com"))
        self.assertFalse(changed)
        self.assertEqual(raw, out)

    def test_non_tracker_url_rejected(self):
        with self.assertRaises(TrackerUrlError):
            transform_tracker_url(
                "** [DHT] **", parse_endpoint("tr.old.com"),
                parse_endpoint("tr.new.com"))

    def test_origin_with_path_rejected(self):
        with self.assertRaises(TrackerUrlError):
            parse_endpoint("https://tr.old.com/announce")


class MaskTests(unittest.TestCase):
    def test_masks_long_path_and_sensitive_query(self):
        raw = ("https://tr.old.com/announce/%s"
               "?passkey=%s&token=%s&x=1" % (KEY, KEY, KEY))
        masked = mask_url(raw)
        self.assertNotIn(KEY, masked)
        self.assertIn("/announce/***", masked)
        self.assertIn("passkey=***", masked)
        self.assertIn("token=***", masked)
        self.assertIn("x=1", masked)

    def test_short_path_not_masked(self):
        raw = "https://tr.old.com/announce/short?x=1"
        self.assertEqual(raw, mask_url(raw))

    def test_fxr_token_shape_masked(self):
        # fxr_ = 前缀 + 32 位：只认字母数字的正则会放行整枚订阅 token
        fxr = "fxr_" + "a" * 32
        masked = mask_url("https://tr.old.com/rss/%s?token=%s" % (fxr, fxr))
        self.assertNotIn(fxr, masked)
        self.assertIn("/rss/***", masked)
        self.assertIn("token=***", masked)

    def test_pseudo_tracker_kept(self):
        self.assertEqual("** [DHT] **", mask_url("** [DHT] **"))

    def test_userinfo_masked(self):
        masked = mask_url(
            "https://user:secret@tr.old.com/announce/%s" % KEY)
        self.assertNotIn("user:secret", masked)
        self.assertIn("***@tr.old.com", masked)


class QbClientTests(unittest.TestCase):
    def test_login_sets_referer_and_cookie(self):
        transport = FakeTransport([
            Response(200, {"set-cookie": "SID=s1; HttpOnly"},
                     b"Ok."),
            Response(200, {}, b'[{"hash": "h1", "name": "n"}]'),
        ])
        client = QbClient(transport, "http://127.0.0.1:8080/",
                          "admin", "pw")
        client.login()
        first = transport.calls[0]
        self.assertEqual(first["method"], "POST")
        self.assertEqual(
            first["url"], "http://127.0.0.1:8080/api/v2/auth/login")
        self.assertEqual(first["headers"]["Referer"],
                         "http://127.0.0.1:8080/")
        self.assertEqual(form(first), {
            "username": ["admin"], "password": ["pw"]})
        self.assertEqual(client.torrents(),
                         [{"hash": "h1", "name": "n"}])
        self.assertEqual(transport.calls[1]["headers"]["Cookie"],
                         "SID=s1")

    def test_login_failure_raises_auth(self):
        transport = FakeTransport([Response(200, {}, b"Fails.")])
        client = QbClient(transport, "http://h", "u", "p")
        with self.assertRaises(AuthError):
            client.login()

    def test_api_key_skips_login(self):
        transport = FakeTransport([Response(200, {}, b"[]")])
        client = QbClient(transport, "http://h", api_key="k")
        client.login()
        self.assertEqual(transport.calls, [])
        self.assertEqual(client.torrents(), [])
        headers = transport.calls[0]["headers"]
        self.assertEqual(headers["Authorization"], "Bearer k")
        self.assertNotIn("Cookie", headers)

    def test_trackers_query_and_edit_shape(self):
        old_url = "https://tr.old.com/announce/%s?passkey=%s" % (
            KEY, KEY)
        new_url = "https://tr.new.com/announce/%s?passkey=%s" % (
            KEY, KEY)
        transport = FakeTransport([
            Response(200, {}, b"[]"),
            Response(200, {}, b""),
        ])
        client = QbClient(transport, "http://h")
        client.trackers("abc")
        client.edit_tracker("abc", old_url, new_url)
        self.assertEqual(
            transport.calls[0]["url"],
            "http://h/api/v2/torrents/trackers?hash=abc")
        self.assertEqual(
            transport.calls[1]["url"],
            "http://h/api/v2/torrents/editTracker")
        body = form(transport.calls[1])
        self.assertEqual(body["hash"], ["abc"])
        self.assertEqual(body["origUrl"], [old_url])
        self.assertEqual(body["newUrl"], [new_url])

    def test_401_is_auth_error(self):
        transport = FakeTransport([Response(401, {}, b"")])
        client = QbClient(transport, "http://h")
        with self.assertRaises(AuthError):
            client.torrents()


class TransmissionTests(unittest.TestCase):
    def test_rpc_url_normalization(self):
        self.assertEqual(
            transmission_rpc_url("http://127.0.0.1:9091"),
            "http://127.0.0.1:9091/transmission/rpc")
        self.assertEqual(
            transmission_rpc_url("http://h/transmission/rpc"),
            "http://h/transmission/rpc")
        self.assertEqual(
            transmission_rpc_url("http://h/transmission/"),
            "http://h/transmission/rpc")

    def test_409_retry_carries_session_and_basic_auth(self):
        payload = {"result": "success",
                   "arguments": {"torrents": []}}
        transport = FakeTransport([
            Response(409, {SESSION_HEADER: "sid1"}, b""),
            Response(200, {}, json.dumps(payload).encode("utf-8")),
        ])
        client = TransmissionClient(transport, "http://h:9091",
                                    "admin", "pw")
        fields = ["id", "hashString", "trackers"]
        args = client.rpc("torrent-get", {"fields": fields})
        self.assertEqual(args, {"torrents": []})
        self.assertEqual(len(transport.calls), 2)
        first, second = transport.calls
        self.assertNotIn(SESSION_HEADER, first["headers"])
        self.assertEqual(second["headers"][SESSION_HEADER], "sid1")
        auth = second["headers"]["Authorization"]
        decoded = base64.b64decode(auth.split()[1]).decode("utf-8")
        self.assertEqual(decoded, "admin:pw")
        sent = json.loads(second["data"].decode("utf-8"))
        self.assertEqual(sent["method"], "torrent-get")
        self.assertEqual(sent["arguments"]["fields"], fields)

    def test_409_twice_is_protocol_error(self):
        transport = FakeTransport([
            Response(409, {SESSION_HEADER: "s1"}, b""),
            Response(409, {SESSION_HEADER: "s2"}, b""),
        ])
        client = TransmissionClient(transport, "http://h")
        with self.assertRaises(ProtocolError):
            client.rpc("torrent-get", {})

    def test_401_is_auth_error(self):
        transport = FakeTransport([Response(401, {}, b"")])
        client = TransmissionClient(transport, "http://h")
        with self.assertRaises(AuthError):
            client.rpc("torrent-get", {})


class FakeQb:
    def __init__(self, trackers, fail_urls=()):
        self.by_hash = trackers
        self.fail_urls = set(fail_urls)
        self.edits = []

    def torrents(self):
        return [{"hash": key, "name": "t-" + key}
                for key in self.by_hash]

    def trackers(self, torrent_hash):
        return [{"url": url} for url in self.by_hash[torrent_hash]]

    def edit_tracker(self, torrent_hash, old_url, new_url):
        if old_url in self.fail_urls:
            raise NetworkError("boom")
        self.edits.append((torrent_hash, old_url, new_url))


class FakeTr:
    def __init__(self, torrents, fail_ids=()):
        self.torrents = torrents
        self.fail_ids = set(fail_ids)
        self.calls = []

    def rpc(self, method, arguments):
        self.calls.append((method, arguments))
        if method == "torrent-get":
            return {"torrents": self.torrents}
        if arguments["ids"][0] in self.fail_ids:
            raise NetworkError("boom")
        return {}


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.old = parse_endpoint("tr.old.com")
        self.new = parse_endpoint("tr.new.com")
        self.old_url = "https://tr.old.com/announce/%s" % KEY
        self.new_url = "https://tr.new.com/announce/%s" % KEY

    def test_qb_dry_run_never_writes(self):
        client = FakeQb({"h1": [self.old_url,
                                "https://other.com/x",
                                "** [DHT] **"]})
        out = io.StringIO()
        with redirect_stdout(out):
            stats = run_qb(client, self.old, self.new, False)
        self.assertEqual(client.edits, [])
        self.assertEqual(
            (stats.torrents, stats.scanned, stats.need,
             stats.success, stats.skipped, stats.failed),
            (1, 3, 1, 0, 2, 0))
        self.assertNotIn(KEY, out.getvalue())
        self.assertIn("[需改]", out.getvalue())

    def test_qb_apply_partial_failure_continues(self):
        old2 = "https://tr.old.com/announce/%s" % ("f" * 32)
        client = FakeQb({
            "h1": [self.old_url],
            "h2": [old2],
        }, fail_urls={old2})
        out = io.StringIO()
        with redirect_stdout(out):
            stats = run_qb(client, self.old, self.new, True)
        self.assertEqual((stats.success, stats.failed), (1, 1))
        self.assertEqual(len(client.edits), 1)
        self.assertNotIn(KEY, out.getvalue())

    def test_tr_dry_run_and_precise_replace(self):
        other = "https://other.com/announce/x"
        torrents = [{
            "id": 7, "hashString": "h7",
            "trackers": [
                {"id": 1, "announce": other, "tier": 0},
                {"id": 2, "announce": self.old_url, "tier": 1},
                {"id": 3, "announce": "** [DHT] **", "tier": 2},
            ],
        }]
        client = FakeTr(torrents)
        with redirect_stdout(io.StringIO()):
            stats = run_tr(client, self.old, self.new, False)
        self.assertEqual(len(client.calls), 1)
        self.assertEqual(client.calls[0][0], "torrent-get")
        self.assertEqual((stats.need, stats.success), (1, 0))

        client = FakeTr(torrents)
        out = io.StringIO()
        with redirect_stdout(out):
            stats = run_tr(client, self.old, self.new, True)
        self.assertEqual((stats.success, stats.failed), (1, 0))
        self.assertNotIn(KEY, out.getvalue())
        method, args = client.calls[1]
        self.assertEqual(method, "torrent-set")
        self.assertEqual(args["ids"], [7])
        self.assertEqual(args["trackerReplace"],
                         [[2, self.new_url]])

    def test_tr_failure_continues_other_torrents(self):
        torrents = [
            {"id": 1, "hashString": "h1", "trackers": [
                {"id": 10, "announce": self.old_url,
                 "tier": 0}]},
            {"id": 2, "hashString": "h2", "trackers": [
                {"id": 20, "announce": self.old_url,
                 "tier": 0}]},
        ]
        client = FakeTr(torrents, fail_ids={1})
        with redirect_stdout(io.StringIO()):
            stats = run_tr(client, self.old, self.new, True)
        self.assertEqual((stats.success, stats.failed), (1, 1))
        self.assertEqual(len(client.calls), 3)


class ExitCodeTests(unittest.TestCase):
    def argv(self, *extra):
        return ["qb", "--url", "http://h", "--old", "tr.old.com",
                "--new", "tr.new.com", "--user", "u",
                "--password", "p"] + list(extra)

    def test_bad_endpoint_returns_2(self):
        with redirect_stderr(io.StringIO()):
            code = main(["qb", "--url", "http://h", "--old",
                         "https://tr.old.com/x", "--new",
                         "tr.new.com"])
        self.assertEqual(code, 2)

    @mock.patch.object(mod, "QbClient")
    @mock.patch.object(mod, "UrllibTransport")
    def test_partial_failure_returns_1(self, _transport, qb_cls):
        qb = FakeQb({
            "h1": ["https://tr.old.com/announce/a"],
            "h2": ["https://tr.old.com/announce/b"],
        }, fail_urls={"https://tr.old.com/announce/b"})
        qb.login = lambda: None
        qb_cls.return_value = qb
        with redirect_stdout(io.StringIO()), \
                redirect_stderr(io.StringIO()):
            code = main(self.argv("--apply"))
        self.assertEqual(code, 1)

    @mock.patch.object(mod, "QbClient")
    @mock.patch.object(mod, "UrllibTransport")
    def test_auth_failure_returns_3(self, _transport, qb_cls):
        qb = FakeQb({})

        def fail():
            raise AuthError("bad")

        qb.login = fail
        qb_cls.return_value = qb
        with redirect_stdout(io.StringIO()), \
                redirect_stderr(io.StringIO()):
            code = main(self.argv("--apply"))
        self.assertEqual(code, 3)

    @mock.patch.object(mod, "QbClient")
    @mock.patch.object(mod, "UrllibTransport")
    def test_network_failure_returns_4(self, _transport, qb_cls):
        qb = FakeQb({})

        def fail():
            raise NetworkError("down")

        qb.login = fail
        qb_cls.return_value = qb
        with redirect_stdout(io.StringIO()), \
                redirect_stderr(io.StringIO()):
            code = main(self.argv("--apply"))
        self.assertEqual(code, 4)

    @mock.patch.object(mod, "QbClient")
    @mock.patch.object(mod, "UrllibTransport")
    def test_dry_run_ok_returns_0(self, _transport, qb_cls):
        qb = FakeQb({"h1": ["https://tr.old.com/announce/a"]})
        qb.login = lambda: None
        qb_cls.return_value = qb
        with redirect_stdout(io.StringIO()):
            code = main(self.argv())
        self.assertEqual(code, 0)


if __name__ == "__main__":
    unittest.main(verbosity=2)
