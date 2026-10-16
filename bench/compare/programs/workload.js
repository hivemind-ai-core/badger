'use strict';
// Plain objects and arrays, Map snapshots, and Node's bounded object-mode stream.
const { PassThrough } = require('node:stream');
const { once } = require('node:events');

function cpu(n, seed) {
    let total = 0;
    for (let i = 0; i < n; i++) {
        let x = (i + seed) % 1000003;
        for (let r = 0; r < 8; r++) {
            x = (x * 17 + 23) % 1000003;
        }
        total += x;
    }
    return total;
}

function records(n, seed) {
    const original = Array.from({ length: n }, (_, id) => ({ id, value: (id + seed) % 1000, active: id % 3 !== 0 }));
    const enriched = original.map(e => ({ ...e, value: e.value * 3 + 7 }));
    return enriched.filter(e => e.active).reduce((a, e) => a + e.value, 0)
        + original.reduce((a, e) => a + e.value, 0);
}

function snapshots(n, seed) {
    const current = new Map();
    const versions = [];
    for (let i = 0; i < n; i++) {
        current.set(i % 256, i + seed);
        if ((i + 1) % 64 === 0) {
            versions.push(new Map(current));
        }
    }
    versions.push(new Map(current));
    let total = 0;
    for (const version of versions) {
        for (const [key, value] of version) {
            total += (key + 1) * value;
        }
    }
    return total;
}

async function handoff(n, seed, batch) {
    // PassThrough has two buffers. Each gets half the 256-element queue budget.
    const stream = new PassThrough({ objectMode: true, highWaterMark: 128 / batch });
    const producer = (async () => {
        for (let start = 0; start < n; start += batch) {
            const value = batch === 1 ? start + seed
                : Array.from({ length: Math.min(batch, n - start) }, (_, k) => start + k + seed);
            if (!stream.write(value)) {
                await once(stream, 'drain');
            }
        }
        stream.end();
    })();
    let total = 0;
    for await (const value of stream) {
        total += batch === 1 ? value : value.reduce((a, x) => a + x, 0);
    }
    await producer;
    return total;
}

async function main() {
    const [name, ...args] = process.argv.slice(2);
    const [n, seed, warmups, repeats] = args.map(Number);
    for (let iteration = 0; iteration < warmups + repeats; iteration++) {
        const started = process.hrtime.bigint();
        const checksum = name === 'handoff' || name === 'batches'
            ? await handoff(n, seed, name === 'handoff' ? 1 : 64)
            : { cpu, records, snapshots }[name](n, seed);
        const elapsed = process.hrtime.bigint() - started;
        console.log(`BENCH ${iteration} ${elapsed} ${checksum}`);
    }
}
main().catch(e => { console.error(e); process.exitCode = 1; });
