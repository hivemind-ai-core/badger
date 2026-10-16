use std::{collections::HashMap, env, sync::mpsc::sync_channel, thread, time::Instant};

#[derive(Clone, Copy)]
struct Event {
    id: i64,
    value: i64,
    active: bool,
}

fn cpu(n: i64, seed: i64) -> i64 {
    (0..n)
        .map(|i| {
            let mut x = (i + seed) % 1_000_003;
            for _ in 0..8 {
                x = (x * 17 + 23) % 1_000_003;
            }
            x
        })
        .sum()
}

fn records(n: i64, seed: i64) -> i64 {
    let original: Vec<_> =
        (0..n).map(|id| Event { id, value: (id + seed) % 1000, active: id % 3 != 0 }).collect();
    let enriched: Vec<_> =
        original.iter().map(|e| Event { id: e.id, value: e.value * 3 + 7, active: e.active }).collect();
    enriched.iter().filter(|e| e.active).map(|e| e.value).sum::<i64>()
        + original.iter().map(|e| e.value).sum::<i64>()
}

fn snapshots(n: i64, seed: i64) -> i64 {
    let mut current = HashMap::new();
    let mut versions = Vec::new();
    for i in 0..n {
        current.insert(i % 256, i + seed);
        if (i + 1) % 64 == 0 {
            versions.push(current.clone());
        }
    }
    versions.push(current.clone());
    versions.iter().flat_map(|m| m.iter()).map(|(k, v)| (k + 1) * v).sum()
}

fn handoff(n: i64, seed: i64, batch: i64) -> i64 {
    thread::scope(|scope| {
        if batch == 1 {
            let (tx, rx) = sync_channel(256);
            let producer = scope.spawn(move || {
                for i in 0..n {
                    tx.send(i + seed).unwrap();
                }
            });
            let sum = rx.iter().sum();
            producer.join().unwrap();
            sum
        } else {
            let (tx, rx) = sync_channel::<Vec<i64>>(4);
            let producer = scope.spawn(move || {
                for start in (0..n).step_by(64) {
                    tx.send((start..(start + 64).min(n)).map(|i| i + seed).collect()).unwrap();
                }
            });
            let sum = rx.iter().flatten().sum();
            producer.join().unwrap();
            sum
        }
    })
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let numbers: Vec<i64> = args[2..].iter().map(|x| x.parse().unwrap()).collect();
    let (n, seed, warmups, repeats) = (numbers[0], numbers[1], numbers[2], numbers[3]);
    for iteration in 0..warmups + repeats {
        let started = Instant::now();
        let checksum = match args[1].as_str() {
            "cpu" => cpu(n, seed),
            "records" => records(n, seed),
            "snapshots" => snapshots(n, seed),
            "handoff" => handoff(n, seed, 1),
            "batches" => handoff(n, seed, 64),
            _ => panic!("unknown workload"),
        };
        let elapsed = started.elapsed().as_nanos();
        println!("BENCH {iteration} {elapsed} {checksum}");
    }
}
