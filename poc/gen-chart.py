#!/usr/bin/env python3
import json
import sys
import os
import math

def load_vegeta(path):
    return [json.loads(l) for l in open(path)]

def parse_ts(ts_str):
    from datetime import datetime
    ts_str = ts_str.rstrip("Z")
    if "+" in ts_str[10:]:
        base = ts_str.rsplit("+", 1)[0]
    elif ts_str[10:].count("-") > 0:
        parts = ts_str[10:].split("-")
        base = ts_str[:10 + len("-".join(parts[:-1]))]
    else:
        base = ts_str
    for fmt in ["%Y-%m-%dT%H:%M:%S.%f", "%Y-%m-%dT%H:%M:%S"]:
        try:
            return datetime.strptime(base[:26], fmt).timestamp()
        except ValueError:
            continue
    return 0

def load_system_metrics(path):
    capacity = {}
    samples = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)
            if d.get("type") == "capacity":
                capacity = d
            else:
                samples.append(d)
    return capacity, samples

def load_request_metrics(path):
    if not os.path.exists(path):
        return []
    return [json.loads(l) for l in open(path) if l.strip()]

def compute_observed(vegeta, requests, capacity, system):
    o = {}
    total = len(vegeta)
    if not total:
        return o

    timestamps = [parse_ts(r["timestamp"]) for r in vegeta]
    duration = max(timestamps) - min(timestamps)
    duration = max(duration, 0.001)
    o["duration"] = round(duration, 3)
    o["total_reqs"] = total

    codes = {}
    for r in vegeta:
        codes[r["code"]] = codes.get(r["code"], 0) + 1
    success = sum(v for k, v in codes.items() if 200 <= k < 300)
    o["success_pct"] = round(success / total * 100, 2)
    o["status_codes"] = codes

    latencies_ms = sorted(r["latency"] / 1e6 for r in vegeta)
    o["mean_ms"] = round(sum(latencies_ms) / len(latencies_ms), 3)
    o["p50_ms"] = round(latencies_ms[int(len(latencies_ms) * 0.5)], 3)
    o["p90_ms"] = round(latencies_ms[int(len(latencies_ms) * 0.9)], 3)
    o["p95_ms"] = round(latencies_ms[int(len(latencies_ms) * 0.95)], 3)
    o["p99_ms"] = round(latencies_ms[int(len(latencies_ms) * 0.99)], 3)
    o["max_ms"] = round(latencies_ms[-1], 3)
    o["rps"] = round(total / duration, 1)

    if requests:
        ok_reqs = [r for r in requests if not r.get("error")]
        if ok_reqs:
            o["proxy_total_us"] = round(sum(r["total_us"] for r in ok_reqs) / len(ok_reqs), 1)
            o["proxy_connect_us"] = round(sum(r["connect_us"] for r in ok_reqs) / len(ok_reqs), 1)
            o["proxy_forward_us"] = round(sum(r["forward_us"] for r in ok_reqs) / len(ok_reqs), 1)
            o["proxy_relay_us"] = round(sum(r["relay_us"] for r in ok_reqs) / len(ok_reqs), 1)

            total_frames = sum(r["frames"] for r in ok_reqs)
            total_req_bytes = sum(r["req_bytes"] for r in ok_reqs)
            total_resp_bytes = sum(r["resp_bytes"] for r in ok_reqs)
            o["total_frames"] = total_frames
            o["total_req_bytes"] = total_req_bytes
            o["total_resp_bytes"] = total_resp_bytes

            o["headers_per_sec"] = round(total_frames / duration, 1)
            o["throughput_bytes"] = total_req_bytes + total_resp_bytes
            o["throughput_mbps"] = round((total_req_bytes + total_resp_bytes) / duration / (1024**2), 3)
            o["frames_per_req"] = round(total_frames / len(ok_reqs), 1) if ok_reqs else 0
            o["resp_bytes_per_req"] = round(total_resp_bytes / len(ok_reqs), 1) if ok_reqs else 0
            o["errors"] = sum(1 for r in requests if r.get("error"))

    if system:
        active_samples = [s for s in system if s.get("active", 0) > 0]
        o["peak_active"] = max((s["active"] for s in system), default=0)
        o["peak_rss"] = max((s["rss"] for s in system), default=0)
        o["peak_cpu"] = round(max((s["cpu"] for s in system), default=0), 1)
        if active_samples:
            interval = 0.1
            o["avg_cpu"] = round(sum(s["cpu"] for s in active_samples) / len(active_samples), 2)
            o["cpu_seconds"] = round(sum(s["cpu"] for s in active_samples) * interval / 100, 4)
        else:
            o["avg_cpu"] = 0
            o["cpu_seconds"] = 0

    o["cpu_count"] = capacity.get("cpu_count", 0)
    o["total_mem"] = capacity.get("total_mem", 0)

    return o

def compute_estimates(o):
    est = {}
    if not o.get("total_reqs"):
        return est

    total = o["total_reqs"]
    duration = o["duration"]
    cpu_count = o.get("cpu_count", 0)

    cpu_seconds = o.get("cpu_seconds", 0)
    if cpu_seconds > 0 and total > 0:
        cpu_per_req_s = cpu_seconds / total
        est["cpu_per_req_us"] = round(cpu_per_req_s * 1e6, 1)
        if cpu_count > 0 and cpu_per_req_s > 0:
            est["max_rps_by_cpu"] = int(cpu_count / cpu_per_req_s)
    else:
        est["cpu_per_req_us"] = 0
        est["max_rps_by_cpu"] = 0

    total_mem = o.get("total_mem", 0)
    peak_rss = o.get("peak_rss", 0)
    peak_active = o.get("peak_active", 0)

    CONN_OVERHEAD = 20 * 1024
    if peak_active >= 50 and peak_rss > 0:
        base_rss = min(s["rss"] for s in system if s.get("rss", 0) > 0) if system else peak_rss
        measured = (peak_rss - base_rss) / peak_active
        CONN_OVERHEAD = max(int(measured), 8192)
        est["per_conn_measured"] = True
    else:
        est["per_conn_measured"] = False
    est["per_conn_bytes"] = CONN_OVERHEAD

    if total_mem > 0:
        est["max_conns_by_mem"] = int(total_mem * 0.80 / CONN_OVERHEAD)
        est["total_mem_gb"] = round(total_mem / (1024**3), 1)
    else:
        est["max_conns_by_mem"] = 0
        est["total_mem_gb"] = 0

    est["cpu_count"] = cpu_count

    max_rps = est.get("max_rps_by_cpu", 0)
    max_conns = est.get("max_conns_by_mem", 0)
    est["practical_max_rps"] = min(max_rps, max_conns) if max_rps and max_conns else max_rps

    frames_per_req = o.get("frames_per_req", 5)
    est["max_headers_per_sec"] = int(est["practical_max_rps"] * frames_per_req)

    resp_per_req = o.get("resp_bytes_per_req", 1000)
    est["max_throughput_gbps"] = round(est["practical_max_rps"] * resp_per_req / (1024**3), 2)

    return est

def fmt_num(n):
    if n >= 1_000_000:
        return f"{n/1_000_000:.1f}M"
    if n >= 1_000:
        return f"{n/1_000:.1f}K"
    return str(int(n))

def fmt_bytes(b):
    if b >= 1024**3:
        return f"{b/1024**3:.2f} GB"
    if b >= 1024**2:
        return f"{b/1024**2:.1f} MB"
    if b >= 1024:
        return f"{b/1024:.1f} KB"
    return f"{b} B"

def generate_html(vegeta, requests, capacity, system, output_path):
    if not vegeta:
        open(output_path, "w").write("<html><body>No data</body></html>")
        return

    o = compute_observed(vegeta, requests, capacity, system)
    est = compute_estimates(o)

    v_t0 = min(parse_ts(r["timestamp"]) for r in vegeta)
    latency_points = [{"t": round(parse_ts(r["timestamp"]) - v_t0, 4), "l": round(r["latency"]/1e6, 3)} for r in vegeta]

    window = 0.5
    max_t = max(p["t"] for p in latency_points)
    tp = []
    t = 0
    while t <= max_t:
        c = sum(1 for p in latency_points if t <= p["t"] < t + window)
        tp.append({"t": round(t + window/2, 3), "rps": round(c/window, 1)})
        t += window

    if system:
        ms = system[0]["t"]
        first_active = next((i for i, m in enumerate(system) if m.get("active", 0) > 0), len(system)//2)
        offset = 0
        if first_active < len(system):
            first_req_t = min(p["t"] for p in latency_points)
            offset = first_req_t - (system[first_active]["t"] - ms)
        am = []
        for m in system:
            ta = round((m["t"] - ms) + offset, 4)
            if -1 <= ta <= max_t + 2:
                am.append({"t": ta, "rss": m["rss"], "cpu": m["cpu"], "active": m["active"]})
    else:
        am = []

    req_timing = []
    if requests:
        ok = [r for r in requests if not r.get("error")]
        if ok:
            r0 = ok[0]["t"]
            first_req_t = min(p["t"] for p in latency_points)
            for r in ok:
                rt = round((r["t"] - r0) + first_req_t, 4)
                if -1 <= rt <= max_t + 2:
                    req_timing.append({"t": rt, "total": r["total_us"], "connect": r["connect_us"], "forward": r["forward_us"], "relay": r["relay_us"]})

    p99_lat = sorted(p["l"] for p in latency_points)[int(len(latency_points) * 0.99)]
    lat_y_max = round(p99_lat * 2, 1)

    s = o.get
    e = est.get

    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Railscale Load Test</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4"></script>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',system-ui,sans-serif;background:#0d1117;color:#c9d1d9;padding:20px;max-width:1400px;margin:0 auto}}
h1{{color:#58a6ff;margin-bottom:4px;font-size:22px}}
h2{{color:#8b949e;font-size:13px;font-weight:500;margin:18px 0 8px;text-transform:uppercase;letter-spacing:0.5px}}
.sub{{color:#8b949e;margin-bottom:14px;font-size:12px}}
.g{{display:grid;gap:8px;margin-bottom:12px}}
.g2{{grid-template-columns:repeat(auto-fit,minmax(120px,1fr))}}
.g3{{grid-template-columns:repeat(auto-fit,minmax(150px,1fr))}}
.s{{background:#161b22;border:1px solid #30363d;border-radius:6px;padding:8px 10px}}
.sv{{font-size:17px;font-weight:600;color:#f0f6fc}}
.sl{{font-size:10px;color:#8b949e;margin-top:1px}}
.e{{border-color:#1f6feb33}}.e .sv{{color:#79c0ff}}.e .sl{{color:#58a6ff}}
.c{{background:#161b22;border:1px solid #30363d;border-radius:6px;padding:12px;margin-bottom:12px}}
.ct{{font-size:12px;color:#8b949e;margin-bottom:6px;font-weight:500}}
canvas{{width:100%!important}}
.n{{font-size:10px;color:#484f58;margin-top:3px}}
</style>
</head>
<body>
<h1>Railscale Load Test</h1>
<div class="sub">{s('total_reqs',0):,} requests over {s('duration',0):.1f}s &middot; {e('cpu_count',0)} CPUs &middot; {e('total_mem_gb',0)} GB RAM</div>

<h2>Observed</h2>
<div class="g g2">
<div class="s"><div class="sv">{s('success_pct',0):.1f}%</div><div class="sl">Success</div></div>
<div class="s"><div class="sv">{s('rps',0):,.0f}/s</div><div class="sl">Requests</div></div>
<div class="s"><div class="sv">{s('mean_ms',0):.2f}ms</div><div class="sl">Mean Latency</div></div>
<div class="s"><div class="sv">{s('p50_ms',0):.2f}ms</div><div class="sl">p50</div></div>
<div class="s"><div class="sv">{s('p95_ms',0):.2f}ms</div><div class="sl">p95</div></div>
<div class="s"><div class="sv">{s('p99_ms',0):.2f}ms</div><div class="sl">p99</div></div>
<div class="s"><div class="sv">{s('max_ms',0):.2f}ms</div><div class="sl">Max</div></div>
<div class="s"><div class="sv">{s('peak_active',0)}</div><div class="sl">Peak Conns</div></div>
<div class="s"><div class="sv">{s('peak_rss',0)/1024/1024:.1f} MB</div><div class="sl">Peak RSS</div></div>
<div class="s"><div class="sv">{s('avg_cpu',0):.1f}%</div><div class="sl">Avg CPU</div></div>
<div class="s"><div class="sv">{fmt_num(s('headers_per_sec',0))}/s</div><div class="sl">Headers Xformed</div></div>
<div class="s"><div class="sv">{s('throughput_mbps',0):.2f} MB/s</div><div class="sl">Throughput</div></div>
</div>

<h2>Proxy Timing (per request avg)</h2>
<div class="g g3">
<div class="s"><div class="sv">{s('proxy_total_us',0):.0f} &micro;s</div><div class="sl">Total</div></div>
<div class="s"><div class="sv">{s('proxy_connect_us',0):.0f} &micro;s</div><div class="sl">Upstream Connect</div></div>
<div class="s"><div class="sv">{s('proxy_forward_us',0):.0f} &micro;s</div><div class="sl">Request Forward</div></div>
<div class="s"><div class="sv">{s('proxy_relay_us',0):.0f} &micro;s</div><div class="sl">Response Relay</div></div>
<div class="s"><div class="sv">{fmt_bytes(s('total_req_bytes',0))}</div><div class="sl">Request Bytes</div></div>
<div class="s"><div class="sv">{fmt_bytes(s('total_resp_bytes',0))}</div><div class="sl">Response Bytes</div></div>
</div>

<h2>Estimated Capacity</h2>
<div class="g g3">
<div class="s e"><div class="sv">{fmt_num(e('practical_max_rps',0))}/s</div><div class="sl">Max Requests</div></div>
<div class="s e"><div class="sv">{fmt_num(e('max_conns_by_mem',0))}</div><div class="sl">Max Conns (RAM)</div></div>
<div class="s e"><div class="sv">{fmt_num(e('max_headers_per_sec',0))}/s</div><div class="sl">Max Headers</div></div>
<div class="s e"><div class="sv">{e('max_throughput_gbps',0):.2f} GB/s</div><div class="sl">Max Throughput</div></div>
<div class="s e"><div class="sv">{e('cpu_per_req_us',0):.1f} &micro;s</div><div class="sl">CPU / Request</div></div>
<div class="s e"><div class="sv">~{e('per_conn_bytes',0)//1024} KB</div><div class="sl">Per-Conn ({'measured' if e('per_conn_measured') else 'est.'})</div></div>
</div>
<div class="n">CPU: integrated avg CPU% over load duration ({s('cpu_seconds',0):.3f} core-sec / {s('total_reqs',0):,} req = {e('cpu_per_req_us',0):.1f}&micro;s) &times; {e('cpu_count',0)} cores. Mem: {e('per_conn_bytes',0)//1024} KB/conn into 80% of {e('total_mem_gb',0)} GB.</div>

<h2 style="margin-top:16px">Charts</h2>

<div class="c"><div class="ct">Latency (ms) &mdash; y-axis clipped at p99&times;2 = {lat_y_max:.1f}ms, outliers above shown at cap</div><canvas id="cLat" height="90"></canvas></div>
<div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
<div class="c"><div class="ct">Throughput (req/s)</div><canvas id="cTp" height="120"></canvas></div>
<div class="c"><div class="ct">Active Connections</div><canvas id="cAct" height="120"></canvas></div>
<div class="c"><div class="ct">RSS Memory (MB)</div><canvas id="cRss" height="120"></canvas></div>
<div class="c"><div class="ct">CPU Usage (%)</div><canvas id="cCpu" height="120"></canvas></div>
<div class="c"><div class="ct">Per-Request: Connect + Forward + Relay (&micro;s)</div><canvas id="cTiming" height="120"></canvas></div>
<div class="c"><div class="ct">Cumulative Requests</div><canvas id="cCum" height="120"></canvas></div>
</div>

<script>
const lat={json.dumps(latency_points)};
const tp={json.dumps(tp)};
const sm={json.dumps(am)};
const rt={json.dumps(req_timing)};
const latYMax={lat_y_max};

const G='#21262d',T='#8b949e';
function sc(yl,extra){{
  const s={{x:{{type:'linear',title:{{display:true,text:'Time (s)',color:T}},grid:{{color:G}},ticks:{{color:T}}}},
    y:{{title:{{display:true,text:yl,color:T}},grid:{{color:G}},ticks:{{color:T}}}}}};
  if(extra)Object.assign(s.y,extra);
  return s;
}}
function mk(id,ds,yl,extra){{
  new Chart(document.getElementById(id),{{type:'line',data:{{datasets:ds}},
    options:{{responsive:true,animation:false,plugins:{{legend:{{display:ds.length>1,labels:{{color:T}}}}}},
    scales:sc(yl,extra),elements:{{point:{{radius:0}},line:{{borderWidth:1.5}}}}}}}});
}}

// Latency scatter with clipped y-axis
new Chart(document.getElementById('cLat'),{{
  type:'scatter',
  data:{{datasets:[{{data:lat.map(p=>({{x:p.t,y:Math.min(p.l,latYMax)}})),
    backgroundColor:'rgba(88,166,255,0.2)',borderColor:'rgba(88,166,255,0.4)',pointRadius:1}}]}},
  options:{{responsive:true,animation:false,plugins:{{legend:{{display:false}}}},
    scales:sc('ms',{{max:latYMax}})}}
}});

mk('cTp',[{{data:tp.map(p=>({{x:p.t,y:p.rps}})),borderColor:'#3fb950',fill:true,backgroundColor:'rgba(63,185,80,0.08)'}}],'req/s');
mk('cAct',[{{data:sm.map(m=>({{x:m.t,y:m.active}})),borderColor:'#d29922',fill:true,backgroundColor:'rgba(210,153,34,0.08)'}}],'connections');
mk('cRss',[{{data:sm.map(m=>({{x:m.t,y:m.rss/1024/1024}})),borderColor:'#f778ba',fill:true,backgroundColor:'rgba(247,120,186,0.08)'}}],'MB');
mk('cCpu',[{{data:sm.map(m=>({{x:m.t,y:m.cpu}})),borderColor:'#bc8cff',fill:true,backgroundColor:'rgba(188,140,255,0.08)'}}],'%');

// Per-request timing stacked
if(rt.length>0){{
  const thin=rt.length>500?rt.filter((_,i)=>i%Math.ceil(rt.length/500)===0):rt;
  mk('cTiming',[
    {{data:thin.map(r=>({{x:r.t,y:r.connect}})),borderColor:'#f97583',label:'Connect',fill:true,backgroundColor:'rgba(249,117,131,0.08)'}},
    {{data:thin.map(r=>({{x:r.t,y:r.forward-r.connect}})),borderColor:'#79c0ff',label:'Forward (excl connect)',fill:true,backgroundColor:'rgba(121,192,255,0.08)'}},
    {{data:thin.map(r=>({{x:r.t,y:r.relay}})),borderColor:'#3fb950',label:'Relay',fill:true,backgroundColor:'rgba(63,185,80,0.08)'}}
  ],'&micro;s');
}} else {{
  document.getElementById('cTiming').parentElement.innerHTML='<div class="ct">No per-request timing (start proxy with RAILSCALE_METRICS_FILE)</div>';
}}

// Cumulative
const sorted=[...lat].sort((a,b)=>a.t-b.t);
let vc=0;const vCum=[];for(const p of sorted){{vc++;if(vc%10===0)vCum.push({{x:p.t,y:vc}});}}
mk('cCum',[
  {{data:vCum,borderColor:'#58a6ff',label:'Vegeta sent',borderDash:[4,2]}},
],'requests');
</script>
</body>
</html>"""

    with open(output_path, "w") as f:
        f.write(html)

if __name__ == "__main__":
    if len(sys.argv) < 4:
        print(f"Usage: {sys.argv[0]} <results.json> <metrics.jsonl> <output.html>")
        sys.exit(1)

    vegeta = load_vegeta(sys.argv[1])
    metrics_file = sys.argv[2]
    capacity, system = load_system_metrics(metrics_file) if os.path.exists(metrics_file) else ({}, [])
    req_file = metrics_file.replace(".jsonl", "-requests.jsonl")
    requests = load_request_metrics(req_file)

    generate_html(vegeta, requests, capacity, system, sys.argv[3])
    n_req = len(requests)
    print(f"Chart written to {sys.argv[3]} ({len(vegeta)} vegeta results, {n_req} proxy request records, {len(system)} system samples)")
