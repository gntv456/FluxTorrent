import http.cookiejar, json, os, subprocess, urllib.error, urllib.request
from argon2 import PasswordHasher
B="http://localhost:8080"
PSQL=["docker","exec","-i","flux-postgres","psql","-U","flux","-d","fluxtorrent","-tAq"]
PW="FluxProbe"+os.urandom(4).hex()
H=PasswordHasher().hash(PW)
def sql(s):
    return subprocess.run(PSQL,input=s,capture_output=True,text=True,encoding="utf-8",errors="replace").stdout.strip()
orig=sql("SELECT pass_hash FROM users WHERE id=1")
esc=orig.replace(chr(39),chr(39)*2)
Q=chr(39)
try:
    sql("UPDATE users SET pass_hash="+Q+H.replace(Q,Q*2)+Q+", must_reset_password=false WHERE id=1;")
    cj=http.cookiejar.CookieJar()
    op=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))
    tok=op.open(B+"/api/v1/me/csrf",timeout=20).headers.get("x-csrf-token")
    rq=urllib.request.Request(B+"/api/v1/auth/login",
        data=json.dumps({"username":"root","password":PW}).encode(),
        headers={"Content-Type":"application/json","X-CSRF-Token":tok},method="POST")
    try:
        r=op.open(rq,timeout=25)
        print("login:",r.status,json.loads(r.read()).get("code"))
    except urllib.error.HTTPError as e:
        print("login FAILED:",e.code,e.read()[:180].decode("utf-8","replace"))
        raise SystemExit(1)
    t2=[c.value for c in cj if c.name=="flux_csrf"]
    t2=t2[0] if t2 else ""
    def post(hdrs):
        q=urllib.request.Request(B+"/api/v1/me/notice-prefs",
            data=json.dumps({"key":"gift","enabled":True}).encode(),
            headers=dict({"Content-Type":"application/json"},**hdrs),method="POST")
        try:
            return op.open(q,timeout=20).status
        except urllib.error.HTTPError as e:
            return e.code
    print("CSRF write WITH token:",post({"X-CSRF-Token":t2}),"(200=放行)")
    print("CSRF write NO token  :",post({}),"(403=闸门在拦)")
    for u in ["/api/v1/torrents?limit=100","/api/v1/torrents?limit=100&alive=0","/api/v1/torrents?limit=100&alive=2"]:
        rr=op.open(B+u,timeout=25)
        d=(json.loads(rr.read()).get("data") or {})
        print("  %-40s items=%s total=%s hidden_dead=%s" % (u,len(d.get("items") or []),d.get("total_estimate"),d.get("hidden_dead")))
    try:
        ir=op.open(B+"/api/v1/crossseed/iyuu/site",timeout=20)
        print("iyuu/site ->",ir.status,ir.read()[:170].decode("utf-8","replace"))
    except urllib.error.HTTPError as e:
        print("iyuu/site ->",e.code,e.read()[:80].decode("utf-8","replace"))
finally:
    sql("UPDATE users SET pass_hash="+Q+esc+Q+" WHERE id=1;")
    print("restored:", sql("SELECT pass_hash FROM users WHERE id=1")==orig)
