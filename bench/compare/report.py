"""Offline HTML report; no network, charting packages, or external assets."""
import html
import json


def render(data):
    esc = lambda value: html.escape(str(value), quote=True)
    meta = data['metadata']
    variants = {v['id']: v for v in data['variants']}

    def milliseconds(stat):
        return f'{stat["median_ns"] / 1e6:,.3f}' if stat else '—'

    sections = []
    for name, workload in data['workloads'].items():
        rows = []
        cells = [r for r in data['results'] if r['workload'] == name]
        cells.sort(key=lambda r: r.get('warm_stats', {}).get('median_ns', float('inf')))
        for row in cells:
            v = variants[row['variant']]
            if row['status'] == 'ok':
                s = row['warm_stats']
                rows.append(f'<tr class="{"ours" if v["family"] == "lang" else ""}"><th scope="row">{esc(v["label"])}</th>'
                            f'<td>{milliseconds(row["wall_stats"])}</td><td>{milliseconds(row["first_stats"])}</td>'
                            f'<td>{milliseconds(s)}</td><td>{s["min_ns"] / 1e6:,.3f}–{s["max_ns"] / 1e6:,.3f}</td>'
                            f'<td class="{"noisy" if s["cv_pct"] > 10 else ""}">{s["cv_pct"]:.1f}%</td><td>verified</td></tr>')
            else:
                rows.append(f'<tr><th scope="row">{esc(v["label"])}</th><td colspan="5">—</td><td>{esc(row["status"])}</td></tr>')
        sections.append(f'<section><h2>{esc(name)}</h2><p>{esc(workload["description"])} '
                        f'<strong>{workload["n"]:,} input items.</strong></p><div class="scroll"><table>'
                        '<thead><tr><th>Runtime / artifact</th><th>Fresh wall ms</th><th>First execution ms</th>'
                        '<th>Warm execution ms</th><th>Warm min–max ms</th><th>Warm CV</th><th>Status</th></tr></thead>'
                        f'<tbody>{"".join(rows)}</tbody></table></div></section>')
    tool_rows = []
    for v in data['variants']:
        build = milliseconds(v.get('build_stats'))
        size = f'{v["artifact_bytes"]:,}' if 'artifact_bytes' in v else '—'
        tool_rows.append(f'<tr><th scope="row">{esc(v["label"])}</th><td>{esc(v["status"])}</td><td>{build}</td><td>{size}</td>'
                         f'<td><pre>{esc(v.get("version", ""))}</pre>{esc(v.get("note", ""))}'
                         f'<details><summary>Command and preparation</summary><pre>{esc(json.dumps({k: v.get(k) for k in ("command", "env", "builds", "setup")}, indent=2))}</pre></details>'
                         f'{"<pre class=error>" + esc(v["reason"]) + "</pre>" if v.get("reason") else ""}</td></tr>')
    errors = [f'<details><summary>{esc(r["variant"])}/{esc(r["workload"])}: {esc(r["status"])}</summary><pre>{esc(r.get("reason", ""))}</pre></details>'
              for r in data['results'] if r['status'] == 'failed']
    args = meta['arguments']
    good = sum(r['status'] == 'ok' for r in data['results'])
    failed = sum(r['status'] == 'failed' for r in data['results']) + sum(v['status'] == 'failed' for v in data['variants'])
    missing = sum(v['status'] == 'missing' for v in data['variants'])
    encoded = json.dumps(data).replace('<', '\\u003c').replace('&', '\\u0026')
    return '''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Language comparison · reproducible benchmark report</title>
<style>
:root{color-scheme:dark;--bg:#0c1421;--panel:#142033;--line:#2b3b53;--text:#ecf2fb;--muted:#a9bad0;--accent:#6ce3b5}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:15px/1.6 system-ui,sans-serif}
main{max-width:1380px;padding:44px 30px 70px;margin:auto}header{padding:15px 0 30px;border-bottom:1px solid var(--line)}
.eyebrow{text-transform:uppercase;letter-spacing:.16em;color:var(--accent);font-size:12px;font-weight:700}
h1{font-size:clamp(30px,4vw,52px);line-height:1.1;letter-spacing:-.035em;margin:16px 0}h2{font-size:23px;margin:0 0 12px}h3{font-size:16px}
p{color:var(--muted);max-width:1100px}strong{color:var(--text)}.pill{display:inline-block;margin:8px 10px 0 0;padding:5px 12px;border:1px solid var(--line);border-radius:30px;font-size:13px}
section{background:var(--panel);border:1px solid var(--line);padding:24px;margin-top:24px;border-radius:12px}.scroll{overflow-x:auto}
table{border-collapse:collapse;width:100%;font-size:13px;font-variant-numeric:tabular-nums}th,td{padding:11px 12px;border-bottom:1px solid var(--line);text-align:right;vertical-align:top;white-space:nowrap}
th:first-child,td:first-child{text-align:left}thead th{color:var(--muted);font-weight:500}tbody th{font-weight:500}.ours{background:#6ce3b50b}.ours th{color:var(--accent)}
.noisy,.error{color:#ffbe7b}pre{white-space:pre-wrap;overflow-wrap:anywhere;text-align:left;font-size:11px;max-width:650px;margin:0 0 6px}
#tools td:last-child{white-space:normal;text-align:left;min-width:300px}details{margin:9px 0}summary{cursor:pointer;color:var(--muted)}
.controls{display:flex;gap:18px;flex-wrap:wrap;margin:20px 0}label{display:flex;flex-direction:column;color:var(--muted);font-size:12px;gap:5px}
select{background:var(--bg);color:var(--text);border:1px solid var(--line);padding:10px;border-radius:6px;font:inherit;min-width:200px}
#chart{width:100%;overflow:auto}svg{display:block;min-width:710px;width:100%;font-family:system-ui,sans-serif}svg text{fill:var(--text);font-size:12px}
.small{font-size:12px;color:var(--muted)}a{color:var(--accent)}code{color:var(--accent)}.notice{border-left:3px solid var(--accent);padding:0 18px}
@media(max-width:650px){main{padding:22px 14px}section{padding:18px 12px}th,td{padding:9px 8px}.controls{gap:10px}}
@media print{:root{color-scheme:light;--bg:white;--panel:white;--line:#ccc;--text:#111;--muted:#444;--accent:#136648}section{break-inside:avoid}.controls{display:none}main{padding:0}svg text{fill:#111}}
</style></head><body><main><header><div class="eyebrow">Language engineering / comparison suite v1</div>
<h1>Same work. Different runtimes.</h1>
<p>Ordinary application code, verified outputs, explicit artifact modes. Compare each workload separately;
these results describe this machine and these programs, not a universal language ranking.</p>
''' + f'<span class="pill">{good} verified cells</span><span class="pill">{failed} failures</span><span class="pill">{missing} unavailable variants</span>' + (
        '<span class="pill error">SMOKE RUN · correctness only, not performance evidence</span>' if args.get('smoke') else '') + f'''
<p class="small">{esc(meta['utc'])} · {esc(meta['cpu'])} · {esc(meta['platform'])}<br>
Compiler revision <code>{esc(meta['revision'][:12])}</code> · {meta['logical_cpus']} logical CPUs · load at start {esc(meta.get('load_average'))}</p>
</header>
<section><h2>Explore the measurements</h2><p>Lower is faster. Bars use milliseconds on a linear scale.
Hover or focus a bar for its measured range. Tables below retain every result.</p>
<div class="controls"><label>Workload<select id="workload"></select></label>
<label>Measurement<select id="metric"><option value="warm_stats">Warmed execution</option><option value="first_stats">First execution</option><option value="wall_stats">Fresh-process wall</option></select></label>
<label>Compare against<select id="baseline"></select></label></div><div id="chart"></div><p id="chart-note" class="small"></p>
<noscript><p>Charts need JavaScript. All measurements and runtime details are available in the tables below.</p></noscript></section>
<section><h2>How to read this run</h2>
<p><strong>Fresh wall:</strong> median of {args.get('fresh')} independent process launches, including startup, source compilation or artifact loading, one workload execution, output, and shutdown.
<strong>First execution:</strong> the internal monotonic timer from those processes, after module loading but including lazy JIT work.
<strong>Warm execution:</strong> {args.get('samples')} iterations in one new process after {args.get('warmups')} warmup iterations; samples are correlated and JIT convergence is not guaranteed.
Every warmup and measurement checksum is checked. Warm CV above 10% is highlighted.</p>
<p>Small-input oracle checks run at n=1/seed=0 and n=65/seed=991 before measurement. Input generation, allocation, queue creation, producer startup, drain, and join are inside the timer.
GC is left enabled with runtime defaults; GC and allocator work can occur between timed iterations. No forced GC, special allocators, SIMD, unsafe fast paths, or language-specific external libraries.</p>
<p>Snapshots have identical observable semantics. Mutable-map languages copy only at snapshot boundaries;
persistent-map languages retain roots and update persistently. Storage, boxing, arithmetic checks, GC, and scheduler implementations differ by design.
Queues have a 256-element nominal budget (four fixed batches), plus producer/consumer working sets. Node streams use two half-size buffers;
high-water marks are backpressure thresholds, not exact capacities. Lua and jank queue cells are unsupported rather than simulated with sequential loops.</p>
<p>Artifact preparation uses {args.get('build_samples')} measured invocation(s), with normal warm compiler caches, excluding installation and host toolchain builds.
Saved MIR and .pyc are intermediate code, JVM classes still JIT, Babashka interprets through SCI, and Rust/Go/C++ native artifacts are machine code.
Current Lang saved MIR lacks the release-only MIR inlining/receive-splitting used by source mode; both use optimized Cranelift code generation. This is a compiler-mode difference, not just a loading difference.
The two Lang worker counts are separate configurations; Go and Java use four scheduler workers. Rust/C++/Python use a producer OS thread; Node uses asynchronous streams; Clojure uses a future pool.</p>
<p class="notice">This suite measures CPU, data transformations, immutable history, and bounded concurrency.
It does not establish correctness or performance guarantees for live reload, resource cleanup under failure, cancellation, durability, authority, HTTP, SQLite, or AI tooling.
No aggregate score mixes these workloads. Shared-host load and short iterations can dominate small differences.</p></section>
{''.join(sections)}
<section id="tools"><h2>Artifacts and toolchains</h2><p>Preparation medians exclude the Lua clock module setup. Artifact bytes exclude shared runtimes and dependencies, so sizes are not deployment-size rankings.
Missing, failed, and unsupported entries are visible and excluded from charts.</p><div class="scroll"><table><thead><tr><th>Runtime / artifact</th><th>Status</th><th>Prepare ms</th><th>Artifact bytes</th><th>Version / method</th></tr></thead><tbody>{''.join(tool_rows)}</tbody></table></div></section>
<section><h2>Evidence and limits</h2><p>The sibling JSON file retains commands, raw stdout/stderr, checksums, warmup times, samples, artifact hashes, source hashes, machine information, and randomized run order seed.
Compilation failures, timeouts, and wrong answers never become speed results. Source-file loading uses the normal warm filesystem cache; this is not a cold-disk test.</p>
{''.join(errors)}<details><summary>Run configuration and provenance</summary><pre>{esc(json.dumps(meta, indent=2))}</pre></details></section>
<p class="small">Methodology and source ports: bench/compare/README.md · Report is standalone and works offline.</p>
</main><script id="data" type="application/json">{encoded}</script>
''' + r'''
<script>
'use strict';
const data = JSON.parse(document.getElementById('data').textContent);
const variantById = Object.fromEntries(data.variants.map(v => [v.id, v]));
const workload = document.getElementById('workload');
const metric = document.getElementById('metric');
const baseline = document.getElementById('baseline');
for (const name of Object.keys(data.workloads)) { workload.add(new Option(name, name)); }
for (const variant of data.variants.filter(v => v.status === 'ready')) { baseline.add(new Option(variant.label, variant.id)); }
if ([...baseline.options].some(o => o.value === 'lang-source-w1')) { baseline.value = 'lang-source-w1'; }
function draw() {
    const rows = data.results.filter(r => r.workload === workload.value && r.status === 'ok' && r[metric.value]);
    rows.sort((a, b) => a[metric.value].median_ns - b[metric.value].median_ns);
    const parent = document.getElementById('chart');
    parent.replaceChildren();
    if (!rows.length) { parent.textContent = 'No verified results yet for this workload.'; return; }
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    const height = rows.length * 34 + 50;
    svg.setAttribute('viewBox', `0 0 1120 ${height}`);
    svg.setAttribute('role', 'img');
    svg.setAttribute('aria-label', `${workload.value}: ${metric.selectedOptions[0].text}, milliseconds, lower is faster`);
    const ref = rows.find(r => r.variant === baseline.value)?.[metric.value].median_ns;
    const maximum = Math.max(...rows.map(r => r[metric.value].max_ns));
    function add(type, attrs, text) {
        const node = document.createElementNS(ns, type);
        for (const [k, v] of Object.entries(attrs)) { node.setAttribute(k, v); }
        if (text !== undefined) { node.textContent = text; }
        svg.append(node); return node;
    }
    for (let i = 0; i <= 4; i++) {
        const x = 315 + i * 150;
        add('line', { x1: x, x2: x, y1: 0, y2: height - 30, stroke: '#2b3b53' });
        add('text', { x, y: height - 8, 'text-anchor': 'middle' }, (maximum / 1e6 * i / 4).toFixed(2));
    }
    rows.forEach((row, index) => {
        const variant = variantById[row.variant];
        const s = row[metric.value];
        const y = index * 34 + 7;
        add('text', { x: 300, y: y + 16, 'text-anchor': 'end' }, variant.label);
        const rect = add('rect', { x: 315, y, width: Math.max(1, s.median_ns / maximum * 600), height: 23, rx: 3,
            fill: variant.family === 'lang' ? '#6ce3b5' : '#739df0', tabindex: 0 });
        const title = document.createElementNS(ns, 'title');
        title.textContent = `${variant.label}: median ${(s.median_ns / 1e6).toFixed(3)} ms, range ${(s.min_ns / 1e6).toFixed(3)}–${(s.max_ns / 1e6).toFixed(3)} ms; n=${s.n}; CV ${s.cv_pct.toFixed(1)}%`;
        rect.append(title);
        add('text', { x: 935, y: y + 16 }, `${(s.median_ns / 1e6).toFixed(3)} ms`);
        if (ref) { add('text', { x: 1035, y: y + 16 }, `${(s.median_ns / ref).toFixed(2)}×`); }
    });
    parent.append(svg);
    document.getElementById('chart-note').textContent = ref
        ? `Ratio = time / ${variantById[baseline.value].label} time. Below 1× is faster than the selected baseline. Bars show medians; hover for ranges. Shared-host results: inspect variability.`
        : 'The selected baseline has no verified result for this workload. Ratios are unavailable.';
}
for (const element of [workload, metric, baseline]) { element.addEventListener('change', draw); }
draw();
</script></body></html>
'''
