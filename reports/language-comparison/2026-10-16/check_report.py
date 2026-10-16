"""Reproduce offline chart interaction and responsive-layout checks."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
report = HERE / 'comparison.html'
evidence = json.loads((HERE / 'comparison.json').read_text())
assert evidence['summary'] == dict(failures=0, ok=98, missing=0)
assert evidence['metadata']['changed_sources_during_run'] == []
checks = r'''
<script>
window.addEventListener('load', () => {
    const checks = [];
    function check(ok, message) {
        if (!ok) { throw new Error(message); }
        checks.push(message);
    }
    check(document.querySelectorAll('#chart svg rect').length === 22, '22 verified CPU chart bars');
    check(!document.querySelector('header').textContent.includes('SMOKE RUN'), 'full-run report');
    const workload = document.querySelector('#workload');
    workload.value = 'handoff'; workload.dispatchEvent(new Event('change'));
    check(document.querySelectorAll('#chart svg rect').length === 16, '16 handoff chart bars; unsupported ports excluded');
    const metric = document.querySelector('#metric');
    metric.value = 'wall_stats'; metric.dispatchEvent(new Event('change'));
    check(document.querySelector('#chart svg').getAttribute('aria-label').includes('Fresh-process wall'), 'measurement control');
    const baseline = document.querySelector('#baseline');
    baseline.value = 'node'; baseline.dispatchEvent(new Event('change'));
    check(document.querySelector('#chart-note').textContent.includes('Node.js'), 'baseline ratios');
    baseline.value = 'lua-source'; baseline.dispatchEvent(new Event('change'));
    check(document.querySelector('#chart-note').textContent.includes('unavailable'), 'unsupported baseline has no ratios');
    workload.value = 'cpu'; workload.dispatchEvent(new Event('change'));
    metric.value = 'warm_stats'; metric.dispatchEvent(new Event('change'));
    baseline.value = 'lang-source-w1'; baseline.dispatchEvent(new Event('change'));
    check(document.documentElement.scrollWidth <= window.innerWidth, 'no page-level horizontal overflow');
    document.body.dataset.qa = 'passed';
    const proof = document.createElement('div'); proof.id = 'qa-proof';
    proof.textContent = JSON.stringify({width: window.innerWidth, checks});
    document.body.append(proof);
});
</script>
'''
chromium = shutil.which('chromium')
assert chromium, 'Chromium is required only to repeat browser QA, not to run benchmarks.'
results = []
with tempfile.TemporaryDirectory(prefix='language-comparison-report-') as temporary:
    temp = Path(temporary)
    target = temp / 'report-qa.html'
    target.write_text(report.read_text().replace('</body>', checks + '</body>'))
    for width, height in ((1440, 1600), (412, 900)):
        command = [chromium, '--headless', '--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage',
                   '--no-first-run', f'--user-data-dir={temp / ("profile-" + str(width))}',
                   f'--window-size={width},{height}', '--dump-dom', target.as_uri()]
        process = subprocess.run(command, capture_output=True, text=True, timeout=30)
        assert process.returncode == 0, process.stderr
        assert 'data-qa="passed"' in process.stdout, process.stderr
        start = process.stdout.index('<div id="qa-proof">') + len('<div id="qa-proof">')
        results.append(json.loads(process.stdout[start:process.stdout.index('</div>', start)]))
    command = [chromium, '--headless', '--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage',
               '--no-first-run', f'--user-data-dir={temp / "screenshot-profile"}', '--window-size=1440,1600',
               f'--screenshot={HERE / "report.png"}', report.as_uri()]
    process = subprocess.run(command, capture_output=True, text=True, timeout=30)
    assert process.returncode == 0, process.stderr
validation = dict(browser=subprocess.run([chromium, '--version'], capture_output=True, text=True, check=True).stdout.strip(),
                  checks=results, benchmark_summary=evidence['summary'],
                  report_sha256=hashlib.sha256(report.read_bytes()).hexdigest(),
                  screenshot_sha256=hashlib.sha256((HERE / 'report.png').read_bytes()).hexdigest())
(HERE / 'validation.json').write_text(json.dumps(validation, indent=2) + '\n')
print(json.dumps(validation, indent=2))
