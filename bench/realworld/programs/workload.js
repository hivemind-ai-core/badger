'use strict';
const { Worker, isMainThread, parentPort } = require('node:worker_threads');
const fs = require('node:fs');

function score(id, seed, rounds) {
    let x = (id + seed) % 1000003;
    for (let r = 0; r < rounds; r++) {
        x = (x * 17 + 23) % 1000003;
    }
    return x;
}
function cpu(n, seed) {
    let total = 0;
    for (let id = 0; id < n; id++) {
        total += score(id, seed, 128);
    }
    return total;
}
function reconcile(n, seed) {
    const discounts = Array.from({length: 1024}, (_, id) => (id * 7 + seed) % 20);
    const rows = Array.from({length: n}, (_, id) => ({
        id, customer: id % 1024, amount: (id * 31 + seed) % 100000,
        active: id % 7 !== 0, note: 'order'
    }));
    const enriched = rows.filter(o => o.active).map(o => ({
        ...o, amount: Math.floor(o.amount * (100 - discounts[o.customer]) / 100)
    }));
    const groups = new Map();
    for (const o of enriched) {
        groups.set(o.customer, (groups.get(o.customer) || 0) + o.amount);
    }
    let total = groups.size;
    for (const [customer, amount] of groups) {
        total += (customer + 1) * amount;
    }
    return total;
}
function jsonOrders(input) {
    let total = 0;
    for (const line of input.split('\n')) {
        if (line !== '') {
            const o = JSON.parse(line);
            // Normal JS runtime validation of the common input schema.
            if (!Number.isSafeInteger(o.id) || !Number.isSafeInteger(o.customer) ||
                !Number.isSafeInteger(o.amount) || typeof o.active !== 'boolean' || typeof o.note !== 'string') {
                throw new Error('invalid order');
            }
            const result = JSON.parse(JSON.stringify({...o, amount: o.amount + Math.floor(o.amount / 5)}));
            if (result.active) {
                total += (result.customer + 1) * result.amount + result.id + Buffer.byteLength(result.note);
            }
        }
    }
    return total;
}
function state(n, seed) {
    const current = new Map();
    const versions = [];
    for (let id = 0; id < n; id++) {
        current.set(id % 1024, (id * 31 + seed) % 100000);
        if ((id + 1) % 256 === 0) {
            versions.push(new Map(current));
        }
    }
    versions.push(new Map(current));
    let total = 0;
    for (const version of versions) {
        for (const [k, v] of version) {
            total += (k + 1) * v;
        }
    }
    return total;
}

// A persistent pool: thread creation is included in the first execution;
// subsequent warm iterations reuse it, just as Go/Badger reuse scheduler threads.
class Pool {
    constructor(count) {
        this.workers = Array.from({length: count}, () => new Worker(__filename));
    }
    run(n, seed, rounds) {
        return new Promise((resolve, reject) => {
            let next = 0;
            let done = 0;
            let total = 0;
            const batches = Math.ceil(n / 64);
            const queue = [];
            const fill = () => {
                while (queue.length < 4 && next < n) {
                    const start = next;
                    next += 64;
                    queue.push(Array.from({length: Math.min(64, n - start)}, (_, k) => start + k));
                }
            };
            fill();
            let settled = false;
            const cleanup = () => {
                for (const worker of this.workers) {
                    worker.removeAllListeners('message');
                    worker.removeAllListeners('error');
                    worker.removeAllListeners('exit');
                }
            };
            const fail = error => {
                if (!settled) {
                    settled = true;
                    cleanup();
                    reject(error);
                }
            };
            const dispatch = worker => {
                if (queue.length > 0) {
                    worker.postMessage({batch: queue.shift(), seed, rounds});
                    fill();
                }
            };
            for (const worker of this.workers) {
                worker.on('error', fail);
                worker.on('exit', code => fail(new Error(`worker exited ${code}`)));
                worker.on('message', value => {
                    total += value;
                    done++;
                    if (done === batches) {
                        settled = true;
                        cleanup();
                        resolve(total);
                    } else {
                        dispatch(worker);
                    }
                });
                dispatch(worker);
            }
        });
    }
    async close() {
        await Promise.all(this.workers.map(w => w.terminate()));
    }
}
async function main() {
    const [name, ...args] = process.argv.slice(2);
    const [n, seed, workers, warmups, repeats] = args.slice(0, 5).map(Number);
    console.log('READY');
    const input = name === 'json' ? fs.readFileSync(args[5], 'utf8') : '';
    let pool;
    try {
        for (let iteration = 0; iteration < warmups + repeats; iteration++) {
            const start = process.hrtime.bigint();
            let checksum;
            if (name === 'messaging' || name === 'pipeline') {
                if (!pool) {
                    pool = new Pool(workers);
                }
                checksum = await pool.run(n, seed, name === 'messaging' ? 2 : 128);
            } else {
                checksum = {cpu, reconcile, state, json: () => jsonOrders(input)}[name](n, seed);
            }
            const elapsed = process.hrtime.bigint() - start;
            console.log(`BENCH ${iteration} ${elapsed} ${checksum}`);
        }
    } finally {
        if (pool) {
            await pool.close();
        }
    }
}
if (isMainThread) {
    main().catch(error => { console.error(error); process.exitCode = 1; });
} else {
    parentPort.on('message', ({batch, seed, rounds}) => {
        let total = 0;
        for (const id of batch) {
            total += score(id, seed, rounds);
        }
        parentPort.postMessage(total);
    });
}
