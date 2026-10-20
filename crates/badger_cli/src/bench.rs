//! `badger bench`: the measurement programs under `bench/`, run and
//! reported.
//!
//! Each program prints its own measurements (`label: value unit`); this
//! collects them, writes them as JSON, and compares them with a
//! baseline written the same way.

use std::path::{Path, PathBuf};

/// One measurement: the program it came from, its label, its value, and
/// its unit.
struct Measurement {
    program: String,
    label: String,
    value: f64,
    unit: String,
}

/// The benchmark programs under `path`: the `.bdg` files in it, not in
/// its subdirectories (those have their own runners).
fn programs(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(path) else { return vec![] };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "bdg"))
        .collect();
    out.sort();
    out
}

/// The measurements a program's output holds: the lines that read
/// `label: value unit`.
fn measurements(program: &str, output: &str) -> Vec<Measurement> {
    let mut out = vec![];
    for line in output.lines() {
        let line = line.trim();
        let Some((label, rest)) = line.split_once(':') else { continue };
        let label = label.trim();
        if label.is_empty() {
            continue;
        }
        let rest = rest.trim();
        let (value, unit) = match rest.split_once(char::is_whitespace) {
            Some((v, u)) => (v.trim(), u.trim()),
            None => (rest, ""),
        };
        let Ok(value) = value.parse::<f64>() else { continue };
        out.push(Measurement {
            program: program.to_string(),
            label: label.to_string(),
            value,
            unit: unit.to_string(),
        });
    }
    out
}

/// The measurements as JSON: `{program: {label: {value, unit}}}`.
fn as_json(all: &[Measurement]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for m in all {
        let entry = serde_json::json!({ "value": m.value, "unit": m.unit });
        let program = out.entry(m.program.clone()).or_insert_with(|| serde_json::json!({}));
        if let Some(labels) = program.as_object_mut() {
            labels.insert(m.label.clone(), entry);
        }
    }
    serde_json::Value::Object(out)
}

/// Write the measurements to a file as JSON.
fn write_json(path: &Path, all: &[Measurement]) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&as_json(all)).map_err(|e| e.to_string())?;
    std::fs::write(path, format!("{text}\n")).map_err(|e| format!("{}: {e}", path.display()))
}

/// Compare the measurements with a baseline: every measurement the
/// baseline holds is reported with its change, and one it does not hold
/// is reported as new.
fn compare_with(baseline: &Path, all: &[Measurement]) -> Result<String, String> {
    let text = std::fs::read_to_string(baseline).map_err(|e| format!("{}: {e}", baseline.display()))?;
    let base: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", baseline.display()))?;
    let mut out = String::new();
    for m in all {
        let before = base
            .get(&m.program)
            .and_then(|p| p.get(&m.label))
            .and_then(|l| l.get("value"))
            .and_then(|v| v.as_f64());
        match before {
            Some(b) if b > 0.0 => {
                let change = (m.value - b) / b * 100.0;
                out.push_str(&format!(
                    "{}: {} {:.3} {} ({:+.1}% against {:.3})\n",
                    m.program, m.label, m.value, m.unit, change, b
                ));
            }
            Some(_) => {
                out.push_str(&format!("{}: {} {:.3} {}\n", m.program, m.label, m.value, m.unit));
            }
            None => {
                out.push_str(&format!(
                    "{}: {} {:.3} {} (new)\n",
                    m.program, m.label, m.value, m.unit
                ));
            }
        }
    }
    Ok(out)
}

/// Run the benchmark programs and report their measurements. With `out`,
/// the measurements are written as JSON; with `baseline`, they are
/// compared with a baseline file written that way.
pub fn run(
    path: &Path,
    workers: Option<usize>,
    out: Option<&Path>,
    baseline: Option<&Path>,
) -> Result<String, String> {
    let programs = programs(path);
    if programs.is_empty() {
        return Err(format!("no benchmark program in {}", path.display()));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut all = vec![];
    let mut text = String::new();
    for p in &programs {
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg("run").arg("--release");
        if let Some(w) = workers {
            cmd.arg("--workers").arg(w.to_string());
        }
        cmd.arg(p);
        let output = cmd.output().map_err(|e| format!("{}: {e}", p.display()))?;
        if !output.status.success() {
            // the program's own diagnostics are the reason
            let why = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(match why.is_empty() {
                true => format!("{}: the program failed", p.display()),
                false => format!("{}: the program failed\n{why}", p.display()),
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let ms = measurements(&p.display().to_string(), &stdout);
        if ms.is_empty() {
            return Err(format!("{}: the program printed no measurement", p.display()));
        }
        // with a baseline to compare, the comparison carries the numbers
        if baseline.is_none() {
            for m in &ms {
                text.push_str(&format!("{}: {} {} {}\n", m.program, m.label, m.value, m.unit));
            }
        }
        all.extend(ms);
    }
    if let Some(f) = out {
        write_json(f, &all)?;
    }
    if let Some(f) = baseline {
        text.push_str(&compare_with(f, &all)?);
    }
    Ok(text)
}
