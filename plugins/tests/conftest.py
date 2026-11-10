"""测试夹具：在**没有 MoviePilot 运行时**的环境里加载插件。

上游插件依赖 MoviePilot 主程序的 `app.*` 包（`app.plugins._PluginBase`、
`app.core.config.settings`、`app.db.site_oper.SiteOper` 等）。本地跑
单元测试时没有 MoviePilot 环境，这里给这些依赖装**最小可用桩**，让
被测的纯逻辑（URL 拼法、请求形状、响应解析）能独立验证。

桩只提供「不报错」所需的最小面：插件里用到但与被测逻辑无关的调用
（数据库、下载器、调度器）一律返回空值。

真实 MoviePilot 环境（上游 CI）下这些 import 会命中真包，本桩不生效 ——
`sys.modules` 注入只在 import 之前发生，且只在 import 失败时启用。
"""

from __future__ import annotations

import importlib.util
import sys
import types
from pathlib import Path

import pytest


# plugins/tests/conftest.py → plugins/ 是上一级
PLUGIN_ROOT = Path(__file__).resolve().parents[1]
# 两个版本目录：
#   crossseed     —— plugins.v2/crossseed（v3.0.3，MoviePilot V2 后端）
#   crossseed_v1  —— plugins/crossseed（v2.4.1，MoviePilot V1 后端）
PLUGIN_DIR = PLUGIN_ROOT / "crossseed"
PLUGIN_DIR_V1 = PLUGIN_ROOT / "crossseed_v1"


def _make_module(name: str) -> types.ModuleType:
    """创建一个空模块并注册进 sys.modules。"""
    module = types.ModuleType(name)
    sys.modules[name] = module
    return module


def _install_app_stubs() -> None:
    """给缺失的 app.* 依赖装最小桩。"""
    # app.core.config.settings
    config_mod = _make_module("app.core.config")
    config_mod.settings = types.SimpleNamespace(
        TZ="Asia/Shanghai",
        USER_AGENT="FluxTorrentTest/1.0",
        PROXY=None,
    )

    # app.log.logger
    log_mod = _make_module("app.log")

    class _Logger:
        """吞掉一切日志调用的哑 logger。"""

        def __getattr__(self, _name):
            def _noop(*args, **kwargs):
                return None

            return _noop

    log_mod.logger = _Logger()

    # app.core.event.eventmanager —— 需要提供 register 装饰器
    event_mod = _make_module("app.core.event")

    class _EventManager:
        """register 装饰器：原样返回被装饰函数。"""

        @staticmethod
        def register(*_args, **_kwargs):
            def _decorator(func):
                return func

            return _decorator

    event_mod.eventmanager = _EventManager()

    # app.db.site_oper.SiteOper
    site_oper_mod = _make_module("app.db.site_oper")

    class _SiteOper:
        """站点仓储桩：任何查询都返回空。"""

        @staticmethod
        def list_order_by_pri():
            return []

        @staticmethod
        def get(_site_id):
            return None

    site_oper_mod.SiteOper = _SiteOper

    # app.helper.sites.SitesHelper
    sites_mod = _make_module("app.helper.sites")

    class _SitesHelper:
        """站点索引桩。"""

        @staticmethod
        def get_indexer(_domain):
            return None

    sites_mod.SitesHelper = _SitesHelper

    # app.helper.torrent.TorrentHelper
    torrent_mod = _make_module("app.helper.torrent")

    class _TorrentHelper:
        """种子助手桩。"""

        @staticmethod
        def download_torrent(**_kwargs):
            return None, None, "", [], "未实现"

    torrent_mod.TorrentHelper = _TorrentHelper

    # app.helper.downloader.DownloaderHelper
    downloader_mod = _make_module("app.helper.downloader")

    class _DownloaderHelper:
        """下载器助手桩。"""

        @staticmethod
        def get_services(_service_type=None, **_kwargs):
            return []

    downloader_mod.DownloaderHelper = _DownloaderHelper

    # app.modules.qbittorrent / app.modules.transmission（v1 版插件用）
    modules_mod = _make_module("app.modules")

    class _Qbittorrent:
        """qB 客户端桩。"""

    class _Transmission:
        """TR 客户端桩。"""

    qb_mod = _make_module("app.modules.qbittorrent")
    qb_mod.Qbittorrent = _Qbittorrent
    tr_mod = _make_module("app.modules.transmission")
    tr_mod.Transmission = _Transmission

    # app.plugins._PluginBase
    plugins_mod = _make_module("app.plugins")

    class _PluginBase:
        """插件基类桩：提供插件实际调用的那几个方法。"""

        def __init__(self):
            self._stub = True

        def get_config(self, _plugin_id=None):
            return None

        def update_config(self, _config, _plugin_id=None):
            return True

        def post_message(self, **_kwargs):
            return None

    plugins_mod._PluginBase = _PluginBase

    # app.schemas
    schemas_mod = _make_module("app.schemas")

    class _NotificationType:
        """通知类型桩。"""

        SiteMessage = "SiteMessage"

    schemas_mod.NotificationType = _NotificationType
    schemas_mod.ServiceInfo = type("ServiceInfo", (), {})

    # app.schemas.types
    types_mod = _make_module("app.schemas.types")

    class _EventType:
        """事件类型桩。"""

        SiteDeleted = "SiteDeleted"

    types_mod.EventType = _EventType

    # app.utils.http.RequestUtils
    http_mod = _make_module("app.utils.http")

    class _RequestUtils:
        """HTTP 工具桩：**真发请求**（但只认桩环境里的 header 约定）。

        上游的 `RequestUtils` 只接受 cookie/ua/proxy，没有自定义头的能力；
        真实 MoviePilot 的 `RequestUtils(headers=...)` 是支持的
        （`app/utils/http.py` 的 `__init__` 第一参就是 headers），
        所以桩按同样的形状实现：吃 headers、照 requests 的语义发请求、
        默认跟随重定向。

        为什么桩要真发：辅种下载链依赖**跨 302 的重定向跟随**
        （站点 dl 口 302 到带临时凭证的下载端点）。返回 None 的死桩
        验证不了这条链。
        """

        def __init__(self, headers=None, ua=None, cookies=None,
                     proxies=None, session=None, timeout=None, **_kwargs):
            self._headers = dict(headers or {})
            if ua:
                self._headers.setdefault("User-Agent", ua)
            self._cookies = cookies
            self._proxies = proxies
            self._timeout = timeout or 20

        def get_res(self, url, params=None, data=None, json=None,
                    allow_redirects=True, raise_exception=False, **kwargs):
            """发 GET 请求并返回响应对象。"""
            import requests as _rq

            headers = kwargs.pop("headers", None) or self._headers
            try:
                return _rq.get(
                    url,
                    params=params,
                    headers=headers,
                    cookies=self._cookies,
                    proxies=self._proxies,
                    timeout=kwargs.pop("timeout", self._timeout),
                    allow_redirects=allow_redirects,
                    verify=False,
                )
            except Exception:  # noqa: BLE001 - 与上游语义一致：异常吞掉返 None
                return None

    http_mod.RequestUtils = _RequestUtils

    # app.utils.string.StringUtils
    string_mod = _make_module("app.utils.string")

    class _StringUtils:
        """字符串工具桩。"""

        @staticmethod
        def generate_random_str(_length=10):
            return "stub"

        @staticmethod
        def get_url_domain(url):
            return url

    string_mod.StringUtils = _StringUtils

    # app.utils.timer.TimerUtils
    timer_mod = _make_module("app.utils.timer")
    timer_mod.TimerUtils = type("TimerUtils", (), {})

    # 补齐包标记，保证 `from app.x.y import Z` 能工作
    for pkg in ("app", "app.core", "app.db", "app.helper", "app.modules",
                "app.schemas", "app.utils"):
        if pkg not in sys.modules:
            mod = _make_module(pkg)
            mod.__path__ = []  # 标记为包


def _load_plugin_module(plugin_dir: Path, mod_name: str):
    """加载插件模块（优先真 MoviePilot 环境，其次桩环境）。

    :param plugin_dir: 插件目录（含 __init__.py）
    :param mod_name: 桩环境下的模块名
    :return: (模块对象, 模块的 import 名)。模块名用于 patch 定位
        （两种环境下的名字不同：真环境是 `app.plugins.crossseed`，
        桩环境是自定的 mod_name）。
    """
    # 1) 真实 MoviePilot 环境（上游 CI）：插件已装在 app/plugins/crossseed
    try:
        import app.plugins.crossseed as mod  # type: ignore
        return mod, "app.plugins.crossseed"
    except Exception:  # noqa: BLE001 - 环境没有就退到桩
        pass

    # 2) 桩环境：直接按文件加载
    _install_app_stubs()
    init_file = plugin_dir / "__init__.py"
    spec = importlib.util.spec_from_file_location(mod_name, init_file)
    module = importlib.util.module_from_spec(spec)
    sys.modules[mod_name] = module
    spec.loader.exec_module(module)
    return module, mod_name


@pytest.fixture(scope="session")
def plugin():
    """提供 v2 版插件里被测的四个符号与模块名。"""
    module, mod_name = _load_plugin_module(
        PLUGIN_DIR, "flux_crossseed_under_test")
    return types.SimpleNamespace(
        FLUX_SITE_NAME=module.FLUX_SITE_NAMES[0],
        FLUX_SITE_NAMES=module.FLUX_SITE_NAMES,
        FLUX_TOKEN_PREFIX=module.FLUX_TOKEN_PREFIX,
        CrossSeedHelper=module.CrossSeedHelper,
        CSSiteConfig=module.CSSiteConfig,
        TorInfo=module.TorInfo,
        module_name=mod_name,
    )


@pytest.fixture(scope="session")
def plugin_v1():
    """提供 v1 版插件里被测的四个符号与模块名。

    v1 版（plugins/crossseed，v2.4.1）与 v2 版（plugins.v2/crossseed，
    v3.0.3）是同一份上游代码的两个分支，FluxTorrent 适配必须**两版
    一致**，所以同一套用例跑两遍。
    """
    module, mod_name = _load_plugin_module(
        PLUGIN_DIR_V1, "flux_crossseed_v1_under_test")
    return types.SimpleNamespace(
        FLUX_SITE_NAME=module.FLUX_SITE_NAMES[0],
        FLUX_SITE_NAMES=module.FLUX_SITE_NAMES,
        FLUX_TOKEN_PREFIX=module.FLUX_TOKEN_PREFIX,
        CrossSeedHelper=module.CrossSeedHelper,
        CSSiteConfig=module.CSSiteConfig,
        TorInfo=module.TorInfo,
        module_name=mod_name,
    )
