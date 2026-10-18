'use strict';
// Exercise generated chart/control logic without a browser or external packages.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const html = fs.readFileSync(path.join(__dirname, 'dashboard.html'), 'utf8');
const code = html.split('<script>')[1].split('</script>')[0];
class Element {
    constructor() { this.children=[]; this.value=''; this.textContent=''; this.innerHTML=''; }
    append(child) {
        this.children.push(child);
        if (this.children.length === 1) { this.value=child.value; }
    }
    replaceChildren(...children) { this.children=children; this.value=children[0]?.value??''; }
}
const summary = v=>({median:v,min:v*.9,max:v*1.1,count:5,cv:.04});
function report(stamp,cohort,revision) {
    const sizes={cpu:100,reconcile:100,json:100,state:100,messaging:100,pipeline:100};
    const cells=Object.keys(sizes).flatMap(workload=>
        (['messaging','pipeline'].includes(workload)?[1,2,4,8]:[1]).flatMap(workers=>
            ['badger','go','node','java'].map(language=>({language,workload,workers,n:100,expected:123,
                summary:{kernel_ns:summary(1000000),first_ns:summary(2000000),
                    startup_ns:summary(4000000),wall_ns:summary(5000000),peak_rss_bytes:summary(1048576),
                    compile_ns:language==='badger'?summary(3000000):null}}))));
    return {schema:1,suite:'realworld-v1',stamp,cohort,revision,status:'complete',smoke:false,
        config:{sizes,workers:[1,2,4,8],processes:5,warmups:3,samples:5},cells,
        builds:{badger:{wall_ns:summary(6e6)},go:{wall_ns:summary(7e6)},node:null,java:{wall_ns:summary(8e6)}},
        host_before:{hostname:'test',cpu:'test CPU',affinity:[0,1]},dirty:'',tools:{},source_hashes:{}};
}
function load(reports) {
    const elements=new Map();
    const get=id=>{
        if (!elements.has(id)) { elements.set(id,new Element()); }
        return elements.get(id);
    };
    get('reports').textContent=JSON.stringify(reports);
    get('metric').value='kernel_ns';
    const context=vm.createContext({document:{getElementById:get,createElement:()=>new Element()}});
    vm.runInContext(code,context);
    return {get,context};
}
const a=report('2026-10-18T00-00-00Z','a','abc123456789');
const b=report('2026-10-19T00-00-00Z','b','def123456789');
const c=report('2026-10-20T00-00-00Z','a','xyz123456789');
const bad={...c,stamp:'2026-10-21T00-00-00Z',status:'failed'};
const smoke={...c,stamp:'2026-10-22T00-00-00Z',smoke:true};
const {get,context}=load([a,b,c,bad,smoke]);
assert.equal(get('cohort').value,'a','most recently measured cohort must be selected');
assert.equal(get('run').value,c.stamp,'latest eligible run must be selected');
assert.match(get('badge').textContent,/3 complete runs · 2 smoke\/failed excluded/);
assert.equal((get('comparison').innerHTML.match(/<svg/g)||[]).length,6);
assert.equal((get('history').innerHTML.match(/<circle/g)||[]).length,8,'two reports × four languages');
for (const metric of ['kernel_ns','first_ns','startup_ns','wall_ns','peak_rss_bytes','compile_ns','build_ns','throughput']) {
    get('metric').value=metric;
    get('metric').onchange();
    assert.doesNotMatch(get('comparison').innerHTML,/NaN|Infinity|undefined/);
    assert.doesNotMatch(get('history').innerHTML,/NaN|Infinity|undefined/);
    if (['compile_ns','build_ns'].includes(metric)) { assert.match(get('comparison').innerHTML,/N\/A/); }
}
get('workers').value='8';
get('workers').onchange();
assert.match(get('comparison').innerHTML,/8 worker\(s\)/);
get('history-workload').value='pipeline';
get('history-workload').onchange();
assert.equal((get('history').innerHTML.match(/<circle/g)||[]).length,8);
assert.match(get('comparison').innerHTML,/viewBox="0 0 380 180"/,'four bars must fit without clipping');
assert.match(get('legend').innerHTML,/Java/);
get('cohort').value='b';
get('cohort').onchange();
assert.equal((get('history').innerHTML.match(/<circle/g)||[]).length,4);
assert.match(get('history-note').textContent,/Baseline recorded/);
const empty=load([bad,smoke]);
assert.match(empty.get('badge').textContent,/no performance baseline/);
console.log('Dashboard checks passed: latest run, cohorts, history, all 8 metrics, workers, exclusions.');

const legacy={...a,cells:a.cells.filter(c=>c.language!=='java')};
const legacyView=load([legacy]);
assert.doesNotMatch(legacyView.get('legend').innerHTML,/Java/,'old reports must not imply Java was measured');
assert.equal((legacyView.get('history').innerHTML.match(/<circle/g)||[]).length,3);
assert.match(legacyView.get('comparison').innerHTML,/viewBox="0 0 380 140"/);
console.log('Legacy three-language report and Java bar height/legend checks passed.');
