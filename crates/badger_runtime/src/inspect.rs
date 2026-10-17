//! Introspection of a running program: tasks (state, wait reason, owner
//! scope), channels (occupancy, staging, waiters), running systems, heap
//! statistics, and the live code version. Served to `badger inspect --connect`
//! by the driver's inspection endpoint and computed on the scheduler thread.

use std::sync::Mutex;

struct Node {
    key: String,
    provider: String,
    /// How many times this node key has been acquired with this provider in
    /// the process (a restarted system or replaced provider counts anew).
    generation: u64,
}

struct System {
    id: u64,
    nodes: Vec<Node>,
    state: &'static str,
}

static SYSTEMS: Mutex<Vec<System>> = Mutex::new(Vec::new());
static NEXT: Mutex<u64> = Mutex::new(1);
static GENERATIONS: Mutex<Vec<((String, String), u64)>> = Mutex::new(Vec::new());

/// A system finished startup with these `(key, provider)` nodes (in start
/// order).
pub fn system_started(nodes: Vec<(String, String)>) -> u64 {
    let id = {
        let mut n = NEXT.lock().unwrap();
        *n += 1;
        *n - 1
    };
    let mut gens = GENERATIONS.lock().unwrap();
    let nodes = nodes
        .into_iter()
        .map(|(key, provider)| {
            let k = (key.clone(), provider.clone());
            let generation = match gens.iter_mut().find(|(x, _)| *x == k) {
                Some((_, g)) => {
                    *g += 1;
                    *g
                }
                None => {
                    gens.push((k, 1));
                    1
                }
            };
            Node { key, provider, generation }
        })
        .collect();
    SYSTEMS.lock().unwrap().push(System { id, nodes, state: "running" });
    id
}

/// The system is halting (`done = false`) or has halted (`done = true`).
pub fn system_stopping(id: u64, done: bool) {
    let mut g = SYSTEMS.lock().unwrap();
    if done {
        g.retain(|s| s.id != id);
    } else if let Some(s) = g.iter_mut().find(|s| s.id == id) {
        s.state = "halting";
    }
}

fn systems_json() -> String {
    let g = SYSTEMS.lock().unwrap();
    let items: Vec<String> = g
        .iter()
        .map(|s| {
            let nodes: Vec<String> = s
                .nodes
                .iter()
                .map(|n| {
                    format!(
                        "{{\"key\":{},\"provider\":{},\"generation\":{}}}",
                        crate::intrinsics_io::json_quote(&n.key),
                        crate::intrinsics_io::json_quote(&n.provider),
                        n.generation
                    )
                })
                .collect();
            format!("{{\"id\":{},\"state\":\"{}\",\"nodes\":[{}]}}", s.id, s.state, nodes.join(","))
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn heap_json() -> String {
    let (objects, bytes, collections) = crate::gc::stats();
    let (total, max) = crate::gc::pauses();
    format!(
        "{{\"objects\":{objects},\"bytes\":{bytes},\"collections\":{collections},\"pause_total_ns\":{total},\"pause_max_ns\":{max}}}"
    )
}

/// JSON for `what`: `tasks`, `channels`, `system`, `heap`, `code`, or `all`.
/// Must run on the scheduler thread.
pub fn snapshot(what: &str) -> Option<String> {
    Some(match what {
        "tasks" => crate::task::snapshot_json(),
        "channels" => crate::channel::snapshot_json(),
        "system" => systems_json(),
        "heap" => heap_json(),
        "code" => format!("{{\"version\":{}}}", crate::live::version()),
        "all" => format!(
            "{{\"code\":{{\"version\":{}}},\"heap\":{},\"system\":{},\"tasks\":{},\"channels\":{}}}",
            crate::live::version(),
            heap_json(),
            systems_json(),
            crate::task::snapshot_json(),
            crate::channel::snapshot_json()
        ),
        _ => return None,
    })
}
