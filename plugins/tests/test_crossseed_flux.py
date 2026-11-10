"""FluxTorrent 站点适配的回归测试。

覆盖四件事（都是「适配我们站点」的实际风险点）：

1. **URL 拼法**：FluxTorrent 走站点原生开放 API，不是上游方言。
2. **响应形状**：两种方言的 `data` 结构完全不同，不可互换 ——
   把上游的 `data.items()` 套到 FluxTorrent 的 `data` 上会拿到一条
   `torrent_id="results"` 的假命中。
3. **凭据位置**：FluxTorrent 的令牌走请求头，不进 body。
4. **上游方言不被改坏**：非 FluxTorrent 站点的行为逐字保持。

符号经 ``plugin`` fixture 注入：真 MoviePilot 环境走真包，
本地无 MoviePilot 时走 conftest 里的桩。
"""

import json
from unittest.mock import patch

import pytest


@pytest.fixture(params=["v2", "v1"])
def env(request, plugin, plugin_v1):
    """把被测符号摊平成局部变量。

    参数化跑**两个版本**（v2 = plugins.v2/crossseed，v1 = plugins/crossseed）——
    它们是同一份上游代码的两个分支，FluxTorrent 适配必须两版一致，
    任何一版漏改都会在真机上表现为「有的能用有的不能」。
    """
    return plugin_v1 if request.param == "v1" else plugin


@pytest.fixture()
def patch_requests(env):
    """给出 requests.post 的 patch 目标路径。

    两种环境下模块名不同，测试里统一用这个 fixture 拿路径，
    避免写死 `app.plugins.crossseed` 在桩环境里 patch 不到。
    """
    return f"{env.module_name}.requests.post"


class _Response:
    """构造 requests.post 所需的最小响应对象。"""

    def __init__(self, payload=None, status_code=200):
        self.status_code = status_code
        self._payload = payload
        self.content = json.dumps(payload).encode() if payload is not None else b""

    def json(self):
        """返回模拟 JSON 结果。"""
        return self._payload

    def raise_for_status(self):
        """非 2xx 时抛异常（与 requests 行为一致）。"""
        if self.status_code >= 400:
            raise RuntimeError(f"HTTP {self.status_code}")


def _flux_site(env, url="https://pt.example.com"):
    """构造一个 FluxTorrent 站点配置。"""
    return env.CSSiteConfig(
        name=env.FLUX_SITE_NAME,
        url=url,
        passkey="fxo_test_token",
    )


# ============ URL 拼法 ============


def test_flux_uses_native_crossseed_api(env):
    """FluxTorrent 查询口应是站点原生开放 API，不是上游方言。"""
    site = _flux_site(env)
    assert site.get_api_url() == \
        "https://pt.example.com/api/v1/crossseed/lookup"


def test_flux_torrent_url_uses_dl_endpoint(env):
    """FluxTorrent 下载口应是 /crossseed/dl/{id}。"""
    site = _flux_site(env)
    assert site.get_torrent_url("12345") == \
        "https://pt.example.com/api/v1/crossseed/dl/12345"


def test_flux_url_tolerates_trailing_slash(env):
    """站点地址带尾斜杠时不能拼出双斜杠。"""
    site = _flux_site(env, url="https://pt.example.com/")
    assert site.get_api_url() == \
        "https://pt.example.com/api/v1/crossseed/lookup"
    assert site.get_torrent_url("1") == \
        "https://pt.example.com/api/v1/crossseed/dl/1"


def test_upstream_dialect_untouched(env):
    """非 FluxTorrent 站点仍走上游方言（不能被适配改坏）。"""
    site = env.CSSiteConfig(name="青蛙", url="https://pt.example.org/")
    assert site.get_api_url() == "https://pt.example.org/api/pieces-hash"
    assert site.get_torrent_url("7") == \
        "https://pt.example.org/download.php?id=7&passkey=None"


def test_hanhhan_dialect_untouched(env):
    """上游对「憨憨」的特殊方言也不能被改坏。"""
    site = env.CSSiteConfig(name="憨憨", url="https://han.example.org/")
    assert site.get_api_url() == \
        "https://han.example.org/nexusapi/pieces-hash"


def test_is_flux_matches_exact_name(env):
    """分派依据是站点名精确匹配，大小写敏感。"""
    assert _flux_site(env).is_flux is True
    assert env.CSSiteConfig(name="fluxtorrent").is_flux is False
    assert env.CSSiteConfig(name="FluxTorrent 2").is_flux is False
    assert env.CSSiteConfig(name="").is_flux is False


# ============ 响应解析 ============


def test_parse_flux_results_reads_results_array(env):
    """FluxTorrent 响应：data.results 是数组，每项含 torrent_id 与 query。"""
    payload = {
        "code": 0,
        "message": "ok",
        "data": {
            "results": [
                {
                    "match": "pieces_sha1",
                    "query": "db6aab38b13a6ae52542471a4a9fef2507973831",
                    "torrent_id": 12345,
                    "name": "Some.Release",
                    "size": 1024,
                    "download_url": "/api/v1/crossseed/dl/12345",
                }
            ]
        },
    }
    infos = env.CrossSeedHelper._parse_flux_results(
        payload, env.FLUX_SITE_NAME)
    assert len(infos) == 1
    assert infos[0].torrent_id == "12345"
    assert infos[0].pieces_hash == \
        "db6aab38b13a6ae52542471a4a9fef2507973831"
    assert infos[0].site_name == env.FLUX_SITE_NAME


def test_parse_flux_results_empty_is_not_error(env):
    """未命中 → 空列表，不是异常。"""
    payload = {"code": 0, "message": "ok", "data": {"results": []}}
    assert env.CrossSeedHelper._parse_flux_results(
        payload, env.FLUX_SITE_NAME) == []


def test_parse_flux_results_ignores_malformed_rows(env):
    """缺 torrent_id 的行直接跳过，不炸。"""
    payload = {
        "code": 0,
        "data": {
            "results": [
                {"match": "pieces_sha1"},
                {"torrent_id": 5, "query": "aa"},
                "not-a-dict",
            ]
        },
    }
    infos = env.CrossSeedHelper._parse_flux_results(payload, "x")
    assert len(infos) == 1
    assert infos[0].torrent_id == "5"


def test_parse_flux_results_never_uses_match_as_fingerprint(env):
    """`match` 是命中类别（pieces_sha1），不能顶替指纹值。

    站点每条 result 里 `match` 说的是"命中的是哪一类指纹"
    （`info_hash_v1` / `pieces_sha1`），`query` 才是原指纹本身。
    拿 `match` 当 pieces_hash 会得到一个本地永远查不到的假指纹，
    上游反查保存路径时就会落空。
    """
    payload = {
        "code": 0,
        "data": {
            "results": [
                {"match": "pieces_sha1", "torrent_id": 7},
                {"match": "info_hash_v1", "query": "aa", "torrent_id": 8},
            ]
        },
    }
    infos = env.CrossSeedHelper._parse_flux_results(payload, "x")
    assert len(infos) == 2
    assert infos[0].torrent_id == "7"
    assert infos[0].pieces_hash == ""
    assert infos[1].torrent_id == "8"
    assert infos[1].pieces_hash == "aa"


def test_parse_flux_results_tolerates_missing_data(env):
    """响应结构异常时返回空列表，不抛异常。"""
    parse = env.CrossSeedHelper._parse_flux_results
    assert parse({}, "x") == []
    assert parse({"data": None}, "x") == []
    assert parse({"data": {}}, "x") == []
    assert parse(None, "x") == []


def test_flux_response_must_not_be_parsed_as_upstream_dialect(env):
    """**核心回归**：FluxTorrent 的 data 不能被当成上游映射解析。

    上游的 `data.items()` 在 FluxTorrent 的 data 上会把 `"results"`
    当成指纹名，产出一条 torrent_id="results" 的假命中。
    """
    payload = {
        "code": 0,
        "data": {
            "results": [
                {"torrent_id": 12345, "query": "abc", "match": "pieces_sha1"}
            ]
        },
    }
    # 上游口径解析 → 假命中
    upstream_style = []
    if isinstance(payload["data"], dict):
        for pieces_hash, torrent_id in payload["data"].items():
            upstream_style.append(
                env.TorInfo.remote("x", pieces_hash, torrent_id))
    assert len(upstream_style) == 1
    # 上游口径把 "results" 当成了**指纹名**，值（results 数组）被当成
    # torrent_id —— 这就是必须分派解析的根因。
    assert upstream_style[0].pieces_hash == "results"

    # FluxTorrent 口径解析 → 真命中
    flux_style = env.CrossSeedHelper._parse_flux_results(payload, "x")
    assert len(flux_style) == 1
    assert flux_style[0].torrent_id == "12345"


# ============ 请求形状 ============


def test_flux_request_uses_header_credential_and_pieces_field(env):
    """FluxTorrent：令牌走 Authorization 头，指纹字段名是 pieces_sha1。"""
    site = _flux_site(env)
    captured = {}

    def _fake_post(url, headers=None, json=None, timeout=None, proxies=None):
        captured["url"] = url
        captured["headers"] = headers
        captured["json"] = json
        return _Response({"code": 0, "data": {"results": []}})

    with patch(f"{env.module_name}.requests.post", side_effect=_fake_post):
        infos, err = env.CrossSeedHelper.get_target_torrent(site, ["aa" * 20])

    assert err is None
    assert infos == []
    assert captured["url"] == \
        "https://pt.example.com/api/v1/crossseed/lookup"
    assert captured["headers"]["Authorization"] == "Token fxo_test_token"
    # 令牌**不进 body**（body 里没有 passkey 键）
    assert "passkey" not in captured["json"]
    assert captured["json"] == {"pieces_sha1": ["aa" * 20]}


def test_upstream_request_keeps_passkey_in_body(env):
    """上游方言：令牌仍在 body 的 passkey 槽位里（不能被改坏）。"""
    site = env.CSSiteConfig(name="青蛙", url="https://pt.example.org/")
    captured = {}

    def _fake_post(url, headers=None, json=None, timeout=None, proxies=None):
        captured["url"] = url
        captured["headers"] = headers
        captured["json"] = json
        return _Response({"data": {}})

    with patch(f"{env.module_name}.requests.post", side_effect=_fake_post):
        env.CrossSeedHelper.get_target_torrent(site, ["bb" * 20])

    assert captured["url"] == "https://pt.example.org/api/pieces-hash"
    assert "Authorization" not in captured["headers"]
    assert captured["json"]["passkey"] is None
    assert captured["json"]["pieces_hash"] == ["bb" * 20]


def test_flux_lookup_returns_hits(env):
    """端到端（打桩）：查询 → 拿到命中。"""
    site = _flux_site(env)

    def _fake_post(url, headers=None, json=None, timeout=None, proxies=None):
        return _Response({
            "code": 0,
            "data": {
                "results": [
                    {
                        "torrent_id": 99,
                        "query": "cc" * 20,
                        "match": "pieces_sha1",
                        "name": "x",
                        "size": 1,
                    }
                ]
            },
        })

    with patch(f"{env.module_name}.requests.post", side_effect=_fake_post):
        infos, err = env.CrossSeedHelper.get_target_torrent(site, ["cc" * 20])

    assert err is None
    assert len(infos) == 1
    assert infos[0].torrent_id == "99"
    assert infos[0].pieces_hash == "cc" * 20
