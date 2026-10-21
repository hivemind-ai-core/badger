//! Explicit MIR optimization pipeline.
//! Run before dumping MIR or code generation; each pass has a name.

pub mod cfg;
pub mod cse;
pub mod globals;
pub mod simplify;
pub mod smt;
pub mod specialize;
pub mod superopt;
pub mod tv;

use crate::mir::Program;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    None,
    Release,
    Aggressive,
    Superopt,
}

impl Level {
    pub fn parse(s: &str) -> Option<Level> {
        match s {
            "none" | "0" => Some(Level::None),
            "release" | "1" => Some(Level::Release),
            "aggressive" | "2" => Some(Level::Aggressive),
            "superopt" | "3" => Some(Level::Superopt),
            _ => None,
        }
    }

    pub fn cranelift(self) -> bool {
        self >= Level::Release
    }

    pub fn name(self) -> &'static str {
        match self {
            Level::None => "none",
            Level::Release => "release",
            Level::Aggressive => "aggressive",
            Level::Superopt => "superopt",
        }
    }
}

/// The pipeline for a level. Passes appear once, in order.
pub fn passes(level: Level) -> Vec<(&'static str, fn(&mut Program) -> usize)> {
    let mut out: Vec<(&'static str, fn(&mut Program) -> usize)> = Vec::new();
    if level == Level::None {
        return out;
    }
    let mut push = |name: &'static str, run: fn(&mut Program) -> usize| out.push((name, run));
    push("globals", globals::fold_constant_globals);
    push("inline", crate::inline::inline);
    push("specialize", specialize::specialize);
    push("sra", program_scalar_replace);
    push("simplify", simplify::simplify);
    push("maps-private", crate::maps::reuse_private_maps);
    push("maps-snapshot", crate::maps::reuse_snapshot_maps);
    push("split", crate::split::split_receives);
    push("niche", crate::niche::elide_local_options);
    push("thread", thread);
    push("enums", crate::enums::scalarize);
    push("simplify2", simplify::simplify);
    push("cse", cse::cse);
    if level >= Level::Aggressive {
        push("inline2", crate::inline::inline);
        push("sra2", program_scalar_replace);
        push("simplify3", simplify::simplify);
        push("cse2", cse::cse);
    }
    if level >= Level::Superopt {
        push("superopt", superopt::superopt);
        push("simplify4", simplify::simplify);
        push("cse3", cse::cse);
    }
    push("reach", crate::reach::prune);
    out
}

/// Jump threading, then the block merges, copies, and dead values it
/// exposes (so a threaded enum object no longer shares a local with the
/// other paths).
fn thread(prog: &mut Program) -> usize {
    if prog.live {
        return 0;
    }
    let mut n = 0;
    let nullary = &prog.origins.nullary;
    for f in prog.funcs.iter_mut() {
        let threaded = cfg::thread_known_branches(f, nullary);
        if threaded > 0 {
            // merge the threaded paths first: the copies to forward are
            // then in one block
            simplify::simplify_func(f);
            simplify::local_copies(f);
            simplify::simplify_func(f);
            n += threaded;
        }
    }
    n
}

fn program_scalar_replace(prog: &mut Program) -> usize {
    let mut n = 0;
    for f in prog.funcs.iter_mut() {
        crate::sra::scalar_replace(f);
        n += 1;
    }
    n
}

fn disabled() -> Vec<String> {
    std::env::var("BADGER_NO_PASS")
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default()
}

/// Run the pipeline for `level` on a program, returning the pass report.
pub fn optimize(prog: &mut Program, level: Level) -> Vec<(&'static str, usize)> {
    pipeline(prog, level, None)
}

/// [`optimize`], handing `check` every pass that ran with the program before
/// and after it (the translation validator's hook, [`tv`]). The program is
/// only copied when a check is given, and `check` cannot change it: the
/// result is the one [`optimize`] produces.
pub fn optimize_checked(
    prog: &mut Program,
    level: Level,
    check: &mut dyn FnMut(&'static str, &Program, &Program),
) -> Vec<(&'static str, usize)> {
    pipeline(prog, level, Some(check))
}

fn pipeline(
    prog: &mut Program,
    level: Level,
    mut check: Option<&mut dyn FnMut(&'static str, &Program, &Program)>,
) -> Vec<(&'static str, usize)> {
    if prog.live || level == Level::None {
        return vec![];
    }
    let off = disabled();
    let dump = std::env::var_os("BADGER_DUMP_MIR_PASSES").is_some();
    let mut report = vec![];
    let mut representation_changed = false;
    for (name, run) in passes(level) {
        let skipped = off.iter().any(|d| {
            d == name || (name.starts_with(d.as_str()) && name[d.len()..].chars().all(|c| c.is_ascii_digit()))
        });
        if skipped {
            continue;
        }
        // The cleanup simplification after the representation passes only
        // has work when one of them changed something; the first
        // simplification already reached a fixpoint.
        if name == "simplify2" && !representation_changed {
            continue;
        }
        let before = check.is_some().then(|| prog.clone());
        let n = run(prog);
        if let (Some(check), Some(before)) = (check.as_mut(), before) {
            check(name, &before, prog);
        }
        if n > 0 && matches!(name, "maps-private" | "maps-snapshot" | "split" | "niche" | "enums") {
            representation_changed = true;
        }
        report.push((name, n));
        if dump {
            eprintln!("=== after {name} ({n}) ===\n{}", crate::mir::dump(prog));
        }
    }
    report
}

/// Whether any optimization pass ran (for timing output).
pub fn enabled(level: Level) -> bool {
    level != Level::None
}
