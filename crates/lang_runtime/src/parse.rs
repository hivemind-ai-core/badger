//! Grammar-string parsers (Earley), used by `std.parse`.
//!
//! Grammar syntax: one rule per line or `;`-separated, `name = alt | alt`,
//! where an alternative is a sequence of rule names, `"literals"`, and
//! `/regexes/` (an empty alternative matches nothing). `%skip /regex/`
//! skips text (e.g. whitespace) before each terminal. The first rule is the
//! start rule. Earley parsing handles left recursion and ambiguity; one
//! tree is returned, and ambiguity is reported (counting stops at two).

use std::collections::HashMap;

use regex::Regex;

#[derive(Clone, Debug)]
pub enum Sym {
    Rule(usize),
    Lit(String),
    Re(String),
}

pub struct Grammar {
    pub names: Vec<String>,
    pub alts: Vec<Vec<Vec<Sym>>>,
    pub skip: Option<Regex>,
    regexes: HashMap<String, Regex>,
}

#[derive(Debug, Clone)]
pub enum Tree {
    Node { rule: String, start: usize, end: usize, children: Vec<Tree> },
    Token { text: String, start: usize, end: usize },
}

pub struct Failure {
    pub offset: usize,
    pub expected: Vec<String>,
}

fn tokenize_rule(text: &str) -> Result<Vec<String>, String> {
    let mut out = vec![];
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '/' {
            let q = c;
            let mut j = i + 1;
            let mut s = String::from(q);
            while j < b.len() && b[j] != q {
                if b[j] == '\\' && j + 1 < b.len() {
                    s.push(b[j]);
                    j += 1;
                }
                s.push(b[j]);
                j += 1;
            }
            if j >= b.len() {
                return Err(format!("unterminated {q} in grammar"));
            }
            s.push(q);
            out.push(s);
            i = j + 1;
        } else if c == '|' || c == '=' {
            out.push(c.to_string());
            i += 1;
        } else if c.is_alphanumeric() || c == '_' {
            let mut j = i;
            while j < b.len() && (b[j].is_alphanumeric() || b[j] == '_') {
                j += 1;
            }
            out.push(b[i..j].iter().collect());
            i = j;
        } else {
            return Err(format!("unexpected `{c}` in grammar"));
        }
    }
    Ok(out)
}

/// Split grammar text into rules at newlines and `;` outside literals and
/// regexes.
fn split_rules(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '\n' | ';' => out.push(std::mem::take(&mut cur)),
                '"' => {
                    quote = Some('"');
                    cur.push(c);
                }
                // a regex starts after `=`, `|`, or whitespace in a rule body
                '/' if cur.contains('=') || cur.trim_start().starts_with("%skip") => {
                    quote = Some('/');
                    cur.push(c);
                }
                _ => cur.push(c),
            },
        }
    }
    out.push(cur);
    out
}

fn unescape(s: &str) -> String {
    let mut o = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => o.push('\n'),
                Some('t') => o.push('\t'),
                Some(x) => o.push(x),
                None => {}
            }
        } else {
            o.push(c);
        }
    }
    o
}

impl Grammar {
    pub fn parse_text(text: &str) -> Result<Grammar, String> {
        let mut names: Vec<String> = vec![];
        let mut bodies: Vec<Vec<String>> = vec![];
        let mut skip = None;
        for raw in split_rules(text) {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("%skip") {
                let r = rest.trim().trim_matches('/');
                skip = Some(Regex::new(&format!("^(?:{r})")).map_err(|e| e.to_string())?);
                continue;
            }
            let toks = tokenize_rule(line)?;
            if toks.len() < 2 || toks[1] != "=" {
                return Err(format!("expected `name = ...`, found `{line}`"));
            }
            let name = toks[0].clone();
            match names.iter().position(|n| *n == name) {
                Some(i) => {
                    bodies[i].push("|".into());
                    bodies[i].extend(toks[2..].iter().cloned());
                }
                None => {
                    names.push(name);
                    bodies.push(toks[2..].to_vec());
                }
            }
        }
        if names.is_empty() {
            return Err("the grammar has no rules".into());
        }
        let mut regexes = HashMap::new();
        let mut alts = vec![];
        for body in &bodies {
            let mut rule_alts = vec![vec![]];
            for t in body {
                if t == "|" {
                    rule_alts.push(vec![]);
                    continue;
                }
                let sym = if t.starts_with('"') {
                    Sym::Lit(unescape(&t[1..t.len() - 1]))
                } else if t.starts_with('/') {
                    let src = t[1..t.len() - 1].replace("\\/", "/");
                    let re = Regex::new(&format!("^(?:{src})")).map_err(|e| e.to_string())?;
                    regexes.insert(src.clone(), re);
                    Sym::Re(src)
                } else {
                    match names.iter().position(|n| n == t) {
                        Some(i) => Sym::Rule(i),
                        None => return Err(format!("undefined rule `{t}`")),
                    }
                };
                rule_alts.last_mut().unwrap().push(sym);
            }
            alts.push(rule_alts);
        }
        Ok(Grammar { names, alts, skip, regexes })
    }

    fn skip_at(&self, input: &str, pos: usize) -> usize {
        match &self.skip {
            Some(r) => r.find(&input[pos..]).map(|m| pos + m.end()).unwrap_or(pos),
            None => pos,
        }
    }

    /// Match a terminal at `pos` (after skipping); returns (start, end).
    fn term(&self, s: &Sym, input: &str, pos: usize) -> Option<(usize, usize)> {
        let p = self.skip_at(input, pos);
        match s {
            Sym::Lit(l) => input[p..].starts_with(l.as_str()).then(|| (p, p + l.len())),
            Sym::Re(src) => {
                self.regexes[src].find(&input[p..]).filter(|m| m.end() > 0).map(|m| (p, p + m.end()))
            }
            Sym::Rule(_) => None,
        }
    }

    fn show(&self, s: &Sym) -> String {
        match s {
            Sym::Lit(l) => format!("{l:?}"),
            Sym::Re(r) => format!("/{r}/"),
            Sym::Rule(i) => self.names[*i].clone(),
        }
    }

    /// Earley recognition. Returns the chart of completed items
    /// `(rule, alt, origin)` per end position, or the furthest failure.
    pub fn parse(&self, input: &str) -> Result<(Tree, bool), Failure> {
        let n = input.len();
        type Item = (usize, usize, usize, usize); // rule, alt, dot, origin
        let mut chart: Vec<Vec<Item>> = vec![vec![]; n + 1];
        let mut done: Vec<std::collections::HashSet<Item>> = vec![Default::default(); n + 1];
        let add = |chart: &mut Vec<Vec<Item>>,
                   done: &mut Vec<std::collections::HashSet<Item>>,
                   k: usize,
                   it: Item| {
            if done[k].insert(it) {
                chart[k].push(it);
            }
        };
        for a in 0..self.alts[0].len() {
            add(&mut chart, &mut done, 0, (0, a, 0, 0));
        }
        let mut furthest = 0;
        let mut expected: Vec<String> = vec![];
        for k in 0..=n {
            let mut i = 0;
            while i < chart[k].len() {
                let (r, a, dot, origin) = chart[k][i];
                let syms = &self.alts[r][a];
                if dot == syms.len() {
                    // complete
                    let waiting: Vec<Item> = chart[origin].clone();
                    for (r2, a2, d2, o2) in waiting {
                        if let Some(Sym::Rule(x)) = self.alts[r2][a2].get(d2)
                            && *x == r
                        {
                            add(&mut chart, &mut done, k, (r2, a2, d2 + 1, o2));
                        }
                    }
                } else {
                    match &syms[dot] {
                        Sym::Rule(x) => {
                            for a2 in 0..self.alts[*x].len() {
                                add(&mut chart, &mut done, k, (*x, a2, 0, k));
                            }
                            // nullable completion already in this set
                            let nullable_done = chart[k].iter().any(|&(r3, a3, d3, o3)| {
                                r3 == *x && o3 == k && d3 == self.alts[r3][a3].len()
                            });
                            if nullable_done {
                                add(&mut chart, &mut done, k, (r, a, dot + 1, origin));
                            }
                        }
                        s => {
                            if let Some((_, e)) = self.term(s, input, k) {
                                add(&mut chart, &mut done, e, (r, a, dot + 1, origin));
                            } else if self.skip_at(input, k) >= furthest {
                                let p = self.skip_at(input, k);
                                if p > furthest || expected.is_empty() {
                                    if p > furthest {
                                        expected.clear();
                                    }
                                    furthest = p;
                                }
                                if p == furthest {
                                    let e = self.show(s);
                                    if !expected.contains(&e) {
                                        expected.push(e);
                                    }
                                }
                            }
                        }
                    }
                }
                i += 1;
            }
        }
        // accept at end of input (allowing trailing skipped text)
        let ends: Vec<usize> = (0..=n).rev().filter(|&k| self.skip_at(input, k) == n).collect();
        for k in ends {
            if chart[k].iter().any(|&(r, a, d, o)| r == 0 && o == 0 && d == self.alts[0][a].len()) {
                let mut memo = HashMap::new();
                let mut counts = HashMap::new();
                let mut visiting = std::collections::HashSet::new();
                if let Some(t) = self.build(0, 0, k, input, &done, &mut memo, &mut visiting) {
                    let ambiguous =
                        self.count(0, 0, k, input, &done, &mut counts, &mut Default::default()) > 1;
                    return Ok((t, ambiguous));
                }
            }
        }
        expected.sort();
        Err(Failure {
            offset: furthest.max(chart.iter().rposition(|c| !c.is_empty()).unwrap_or(0)).min(n),
            expected,
        })
    }

    fn completed(
        &self,
        done: &[std::collections::HashSet<(usize, usize, usize, usize)>],
        r: usize,
        a: usize,
        o: usize,
        e: usize,
    ) -> bool {
        done[e].contains(&(r, a, self.alts[r][a].len(), o))
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        &self,
        r: usize,
        start: usize,
        end: usize,
        input: &str,
        done: &[std::collections::HashSet<(usize, usize, usize, usize)>],
        memo: &mut HashMap<(usize, usize, usize), Option<Tree>>,
        visiting: &mut std::collections::HashSet<(usize, usize, usize)>,
    ) -> Option<Tree> {
        if let Some(t) = memo.get(&(r, start, end)) {
            return t.clone();
        }
        if !visiting.insert((r, start, end)) {
            return None;
        }
        let mut result = None;
        for a in 0..self.alts[r].len() {
            if !self.completed(done, r, a, start, end) {
                continue;
            }
            if let Some(children) = self.seq(&self.alts[r][a], 0, start, end, input, done, memo, visiting) {
                result = Some(Tree::Node { rule: self.names[r].clone(), start, end, children });
                break;
            }
        }
        visiting.remove(&(r, start, end));
        memo.insert((r, start, end), result.clone());
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn seq(
        &self,
        syms: &[Sym],
        i: usize,
        pos: usize,
        end: usize,
        input: &str,
        done: &[std::collections::HashSet<(usize, usize, usize, usize)>],
        memo: &mut HashMap<(usize, usize, usize), Option<Tree>>,
        visiting: &mut std::collections::HashSet<(usize, usize, usize)>,
    ) -> Option<Vec<Tree>> {
        if i == syms.len() {
            return (pos == end).then(Vec::new);
        }
        match &syms[i] {
            Sym::Rule(x) => {
                for mid in (pos..=end).rev() {
                    let any = (0..self.alts[*x].len()).any(|a| self.completed(done, *x, a, pos, mid));
                    if !any {
                        continue;
                    }
                    if let Some(t) = self.build(*x, pos, mid, input, done, memo, visiting)
                        && let Some(mut rest) = self.seq(syms, i + 1, mid, end, input, done, memo, visiting)
                    {
                        rest.insert(0, t);
                        return Some(rest);
                    }
                }
                None
            }
            s => {
                let (st, e) = self.term(s, input, pos)?;
                let mut rest = self.seq(syms, i + 1, e, end, input, done, memo, visiting)?;
                rest.insert(0, Tree::Token { text: input[st..e].to_string(), start: st, end: e });
                Some(rest)
            }
        }
    }

    /// Number of derivations (saturating at 2).
    #[allow(clippy::too_many_arguments)]
    fn count(
        &self,
        r: usize,
        start: usize,
        end: usize,
        input: &str,
        done: &[std::collections::HashSet<(usize, usize, usize, usize)>],
        memo: &mut HashMap<(usize, usize, usize), u32>,
        visiting: &mut std::collections::HashSet<(usize, usize, usize)>,
    ) -> u32 {
        if let Some(c) = memo.get(&(r, start, end)) {
            return *c;
        }
        if !visiting.insert((r, start, end)) {
            return 0;
        }
        let mut total = 0u32;
        for a in 0..self.alts[r].len() {
            if self.completed(done, r, a, start, end) {
                total += self.count_seq(&self.alts[r][a], 0, start, end, input, done, memo, visiting);
            }
            if total >= 2 {
                break;
            }
        }
        visiting.remove(&(r, start, end));
        let total = total.min(2);
        memo.insert((r, start, end), total);
        total
    }

    #[allow(clippy::too_many_arguments)]
    fn count_seq(
        &self,
        syms: &[Sym],
        i: usize,
        pos: usize,
        end: usize,
        input: &str,
        done: &[std::collections::HashSet<(usize, usize, usize, usize)>],
        memo: &mut HashMap<(usize, usize, usize), u32>,
        visiting: &mut std::collections::HashSet<(usize, usize, usize)>,
    ) -> u32 {
        if i == syms.len() {
            return (pos == end) as u32;
        }
        match &syms[i] {
            Sym::Rule(x) => {
                let mut t = 0;
                for mid in pos..=end {
                    let c = self.count(*x, pos, mid, input, done, memo, visiting);
                    if c > 0 {
                        t += c * self.count_seq(syms, i + 1, mid, end, input, done, memo, visiting);
                    }
                    if t >= 2 {
                        return 2;
                    }
                }
                t
            }
            s => match self.term(s, input, pos) {
                Some((_, e)) => self.count_seq(syms, i + 1, e, end, input, done, memo, visiting),
                None => 0,
            },
        }
    }
}

/// Completed Earley items per end position: `(rule, alt, dot, origin)`.
type Done = Vec<std::collections::HashSet<(usize, usize, usize, usize)>>;
/// A forest node key `(rule, start, end)`.
type NodeKey = (usize, usize, usize);
/// Forest nodes with their packed alternatives.
type Nodes = [(NodeKey, Vec<Vec<Child>>)];

/// Budget for packed alternatives per forest node and for tree enumeration.
const PACK_BUDGET: usize = 64;

/// A child in a packed alternative: a forest node or a token.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Child {
    Node(usize, usize, usize),
    Token(usize, usize),
}

impl Grammar {
    /// Recognize and return the chart of completed items, or the failure.
    fn recognize(&self, input: &str) -> Result<(usize, Done), Failure> {
        // reuse `parse` for the failure report; rebuild the chart here
        let n = input.len();
        type Item = (usize, usize, usize, usize);
        let mut chart: Vec<Vec<Item>> = vec![vec![]; n + 1];
        let mut done: Vec<std::collections::HashSet<Item>> = vec![Default::default(); n + 1];
        let add = |chart: &mut Vec<Vec<Item>>,
                   done: &mut Vec<std::collections::HashSet<Item>>,
                   k: usize,
                   it: Item| {
            if done[k].insert(it) {
                chart[k].push(it);
            }
        };
        for a in 0..self.alts[0].len() {
            add(&mut chart, &mut done, 0, (0, a, 0, 0));
        }
        for k in 0..=n {
            let mut i = 0;
            while i < chart[k].len() {
                let (r, a, dot, origin) = chart[k][i];
                let syms = &self.alts[r][a];
                if dot == syms.len() {
                    let waiting: Vec<Item> = chart[origin].clone();
                    for (r2, a2, d2, o2) in waiting {
                        if let Some(Sym::Rule(x)) = self.alts[r2][a2].get(d2)
                            && *x == r
                        {
                            add(&mut chart, &mut done, k, (r2, a2, d2 + 1, o2));
                        }
                    }
                } else {
                    match &syms[dot] {
                        Sym::Rule(x) => {
                            for a2 in 0..self.alts[*x].len() {
                                add(&mut chart, &mut done, k, (*x, a2, 0, k));
                            }
                            let nullable_done = chart[k].iter().any(|&(r3, a3, d3, o3)| {
                                r3 == *x && o3 == k && d3 == self.alts[r3][a3].len()
                            });
                            if nullable_done {
                                add(&mut chart, &mut done, k, (r, a, dot + 1, origin));
                            }
                        }
                        s => {
                            if let Some((_, e)) = self.term(s, input, k) {
                                add(&mut chart, &mut done, e, (r, a, dot + 1, origin));
                            }
                        }
                    }
                }
                i += 1;
            }
        }
        let ends: Vec<usize> = (0..=n).rev().filter(|&k| self.skip_at(input, k) == n).collect();
        for k in ends {
            if chart[k].iter().any(|&(r, a, d, o)| r == 0 && o == 0 && d == self.alts[0][a].len()) {
                return Ok((k, done));
            }
        }
        match self.parse(input) {
            Err(f) => Err(f),
            Ok(_) => Err(Failure { offset: n, expected: vec![] }),
        }
    }

    /// Every way to split `syms[i..]` over `pos..end` (bounded).
    #[allow(clippy::too_many_arguments)]
    fn splits(
        &self,
        syms: &[Sym],
        i: usize,
        pos: usize,
        end: usize,
        input: &str,
        done: &[std::collections::HashSet<(usize, usize, usize, usize)>],
        out: &mut Vec<Vec<Child>>,
        prefix: &mut Vec<Child>,
    ) {
        if out.len() >= PACK_BUDGET {
            return;
        }
        if i == syms.len() {
            if pos == end {
                out.push(prefix.clone());
            }
            return;
        }
        match &syms[i] {
            Sym::Rule(x) => {
                for mid in pos..=end {
                    if (0..self.alts[*x].len()).any(|a| self.completed(done, *x, a, pos, mid)) {
                        prefix.push(Child::Node(*x, pos, mid));
                        self.splits(syms, i + 1, mid, end, input, done, out, prefix);
                        prefix.pop();
                    }
                }
            }
            s => {
                if let Some((st, e)) = self.term(s, input, pos) {
                    prefix.push(Child::Token(st, e));
                    self.splits(syms, i + 1, e, end, input, done, out, prefix);
                    prefix.pop();
                }
            }
        }
    }

    /// The shared packed parse forest: each node `(rule, start, end)` lists
    /// its packed alternatives (child sequences), sharing subnodes.
    /// Returns the nodes, the root, whether any node has several
    /// alternatives, and whether a node derives itself (a cycle: infinitely
    /// many trees).
    #[allow(clippy::type_complexity)]
    fn forest_nodes(
        &self,
        input: &str,
    ) -> Result<(Vec<((usize, usize, usize), Vec<Vec<Child>>)>, (usize, usize, usize), bool, bool), Failure>
    {
        let (end, done) = self.recognize(input)?;
        let root = (0usize, 0usize, end);
        let mut nodes: Vec<((usize, usize, usize), Vec<Vec<Child>>)> = vec![];
        let mut index: HashMap<(usize, usize, usize), usize> = HashMap::new();
        let mut stack = vec![root];
        let mut ambiguous = false;
        while let Some(key) = stack.pop() {
            if index.contains_key(&key) {
                continue;
            }
            let (r, st, en) = key;
            let mut packed = vec![];
            for a in 0..self.alts[r].len() {
                if self.completed(&done, r, a, st, en) {
                    self.splits(&self.alts[r][a], 0, st, en, input, &done, &mut packed, &mut vec![]);
                }
            }
            ambiguous |= packed.len() > 1;
            for alt in &packed {
                for c in alt {
                    if let Child::Node(x, s2, e2) = c {
                        stack.push((*x, *s2, *e2));
                    }
                }
            }
            index.insert(key, nodes.len());
            nodes.push((key, packed));
        }
        // a cycle: a node reachable from itself
        let mut cyclic = false;
        let mut state: HashMap<(usize, usize, usize), u8> = HashMap::new();
        fn dfs(
            k: (usize, usize, usize),
            nodes: &[((usize, usize, usize), Vec<Vec<Child>>)],
            index: &HashMap<(usize, usize, usize), usize>,
            state: &mut HashMap<(usize, usize, usize), u8>,
        ) -> bool {
            match state.get(&k) {
                Some(1) => return true,
                Some(2) => return false,
                _ => {}
            }
            state.insert(k, 1);
            for alt in &nodes[index[&k]].1 {
                for c in alt {
                    if let Child::Node(x, s, e) = c
                        && dfs((*x, *s, *e), nodes, index, state)
                    {
                        return true;
                    }
                }
            }
            state.insert(k, 2);
            false
        }
        if dfs(root, &nodes, &index, &mut state) {
            cyclic = true;
        }
        Ok((nodes, root, ambiguous, cyclic))
    }

    /// The forest as JSON: `{root, ambiguous, cyclic, nodes: [{id, rule,
    /// start, end, alternatives: [[{node: id} | {token, start, end}]]}]}`.
    pub fn forest_json(&self, input: &str) -> Result<serde_json::Value, Failure> {
        let (nodes, root, ambiguous, cyclic) = self.forest_nodes(input)?;
        let ids: HashMap<(usize, usize, usize), usize> =
            nodes.iter().enumerate().map(|(i, (k, _))| (*k, i)).collect();
        let child = |c: &Child| match c {
            Child::Node(x, s, e) => serde_json::json!({ "node": ids[&(*x, *s, *e)] }),
            Child::Token(s, e) => serde_json::json!({ "token": &input[*s..*e], "start": s, "end": e }),
        };
        let ns: Vec<serde_json::Value> = nodes
            .iter()
            .enumerate()
            .map(|(i, ((r, s, e), packed))| {
                serde_json::json!({
                    "id": i, "rule": self.names[*r], "start": s, "end": e,
                    "alternatives": packed.iter().map(|alt| alt.iter().map(child).collect::<Vec<_>>()).collect::<Vec<_>>(),
                })
            })
            .collect();
        Ok(serde_json::json!({ "root": ids[&root], "ambiguous": ambiguous, "cyclic": cyclic, "nodes": ns }))
    }

    /// Up to `limit` distinct trees, enumerated from the forest; a cycle is
    /// followed at most once per path and reported. Returns the trees,
    /// whether enumeration stopped at the limit, and whether a cycle exists.
    pub fn trees(&self, input: &str, limit: usize) -> Result<(Vec<Tree>, bool, bool), Failure> {
        let (nodes, root, _, cyclic) = self.forest_nodes(input)?;
        let index: HashMap<(usize, usize, usize), usize> =
            nodes.iter().enumerate().map(|(i, (k, _))| (*k, i)).collect();
        let mut truncated = false;
        #[allow(clippy::too_many_arguments)]
        fn expand(
            g: &Grammar,
            k: NodeKey,
            input: &str,
            nodes: &Nodes,
            index: &HashMap<(usize, usize, usize), usize>,
            path: &mut Vec<(usize, usize, usize)>,
            limit: usize,
            truncated: &mut bool,
        ) -> Vec<Tree> {
            if path.contains(&k) {
                return vec![];
            }
            path.push(k);
            let mut out = vec![];
            for alt in &nodes[index[&k]].1 {
                // cartesian product of the children's trees, bounded
                let mut partial: Vec<Vec<Tree>> = vec![vec![]];
                for c in alt {
                    let options: Vec<Tree> = match c {
                        Child::Token(s, e) => {
                            vec![Tree::Token { text: input[*s..*e].to_string(), start: *s, end: *e }]
                        }
                        Child::Node(x, s, e) => {
                            expand(g, (*x, *s, *e), input, nodes, index, path, limit, truncated)
                        }
                    };
                    let mut next = vec![];
                    'outer: for p in &partial {
                        for o in &options {
                            if next.len() >= limit {
                                *truncated = true;
                                break 'outer;
                            }
                            let mut q = p.clone();
                            q.push(o.clone());
                            next.push(q);
                        }
                    }
                    partial = next;
                }
                for children in partial {
                    if out.len() >= limit {
                        *truncated = true;
                        break;
                    }
                    out.push(Tree::Node { rule: g.names[k.0].clone(), start: k.1, end: k.2, children });
                }
            }
            path.pop();
            out
        }
        let trees = expand(self, root, input, &nodes, &index, &mut vec![], limit.max(1), &mut truncated);
        Ok((trees, truncated, cyclic))
    }

    /// Parse with recovery: at each failure, the shortest region after the
    /// failure point whose deletion lets the parse get further is skipped and
    /// recorded as an error. Tree offsets refer to the original input.
    pub fn parse_recovering(&self, input: &str, max_errors: usize) -> Result<(Tree, Vec<Failure>), Failure> {
        let mut text = input.to_string();
        // (position in edited text, removed length), in edit order
        let mut removed: Vec<(usize, usize)> = vec![];
        let mut errors = vec![];
        loop {
            match self.parse(&text) {
                Ok((t, _)) => {
                    let map = |p: usize| -> usize {
                        let mut q = p;
                        for (at, len) in removed.iter().rev() {
                            if q >= *at {
                                q += len;
                            }
                        }
                        q
                    };
                    fn remap(t: Tree, map: &dyn Fn(usize) -> usize) -> Tree {
                        match t {
                            Tree::Node { rule, start, end, children } => Tree::Node {
                                rule,
                                start: map(start),
                                end: map(end),
                                children: children.into_iter().map(|c| remap(c, map)).collect(),
                            },
                            Tree::Token { text, start, end } => {
                                Tree::Token { text, start: map(start), end: map(end) }
                            }
                        }
                    }
                    return Ok((remap(t, &map), errors));
                }
                Err(f) => {
                    if errors.len() >= max_errors || f.offset >= text.len() {
                        return Err(f);
                    }
                    let at = f.offset;
                    let original = {
                        let mut q = at;
                        for (a, len) in removed.iter().rev() {
                            if q >= *a {
                                q += len;
                            }
                        }
                        q
                    };
                    // the shortest deletion after which parsing gets further
                    let mut cut = None;
                    let mut len = 1;
                    while at + len <= text.len() {
                        if !text.is_char_boundary(at + len) {
                            len += 1;
                            continue;
                        }
                        let candidate = format!("{}{}", &text[..at], &text[at + len..]);
                        match self.parse(&candidate) {
                            Ok(_) => {
                                cut = Some(len);
                                break;
                            }
                            Err(g) if g.offset > at => {
                                cut = Some(len);
                                break;
                            }
                            _ => {}
                        }
                        len += 1;
                    }
                    let Some(len) = cut else { return Err(f) };
                    errors.push(Failure { offset: original, expected: f.expected });
                    text = format!("{}{}", &text[..at], &text[at + len..]);
                    removed.push((at, len));
                }
            }
        }
    }
}

pub fn tree_json(t: &Tree) -> serde_json::Value {
    match t {
        Tree::Node { rule, start, end, children } => serde_json::json!({
            "rule": rule, "start": start, "end": end,
            "children": children.iter().map(tree_json).collect::<Vec<_>>(),
        }),
        Tree::Token { text, start, end } => serde_json::json!({ "token": text, "start": start, "end": end }),
    }
}

// ------------------------------------------------------------------ intrinsics

use crate::desc::TypeDesc;
use crate::gc::Roots;
use crate::intrinsics::{Targs, rdesc};
use crate::string::{new_string, str_of};
use crate::task::TaskCtx;

static GRAMMARS: std::sync::Mutex<Vec<std::sync::Arc<Grammar>>> = std::sync::Mutex::new(Vec::new());

/// `Result<Int, String>`: compile a grammar.
pub extern "C" fn parse_grammar(_c: *mut TaskCtx, _t: Targs, ret: *const TypeDesc, text: u64) -> u64 {
    match Grammar::parse_text(str_of(text)) {
        Ok(g) => {
            let mut gs = GRAMMARS.lock().unwrap();
            gs.push(std::sync::Arc::new(g));
            crate::make_variant(rdesc(ret), "ok", &[(gs.len() - 1) as u64])
        }
        Err(e) => {
            let m = new_string(&e);
            crate::make_variant(rdesc(ret), "err", &[m])
        }
    }
}

/// `Result<{tree: Data, ambiguous: Bool}, Failure>`
pub extern "C" fn parse_run(_c: *mut TaskCtx, _t: Targs, ret: *const TypeDesc, g: u64, input: u64) -> u64 {
    let ret = rdesc(ret);
    let grammar = GRAMMARS.lock().unwrap()[g as usize].clone();
    let text = str_of(input).to_string();
    let r = Roots::new();
    match grammar.parse(&text) {
        Ok((tree, ambiguous)) => {
            let ok_desc = ret.variant(0).unwrap().fields[0].desc;
            let data_desc = ok_desc.field("tree").unwrap().desc;
            let d = r.add(crate::codec::data_of(data_desc, &tree_json(&tree)));
            let v = r.add(crate::make_record(ok_desc, &[("tree", d), ("ambiguous", ambiguous as u64)]));
            crate::make_variant(ret, "ok", &[v])
        }
        Err(f) => {
            let err_desc = ret.variant(1).unwrap().fields[0].desc;
            let (line, col) = {
                let before = &text[..f.offset.min(text.len())];
                (
                    before.matches('\n').count() + 1,
                    before.len() - before.rfind('\n').map(|p| p + 1).unwrap_or(0) + 1,
                )
            };
            let list_desc = err_desc.field("expected").unwrap().desc;
            let l = r.add(crate::list::new(list_desc));
            for e in &f.expected {
                let w = r.add(new_string(e));
                crate::list::push(l, w);
            }
            let v = r.add(crate::make_record(
                err_desc,
                &[
                    ("offset", f.offset as u64),
                    ("line", line as u64),
                    ("column", col as u64),
                    ("expected", l),
                ],
            ));
            crate::make_variant(ret, "err", &[v])
        }
    }
}

fn failure_value(err_desc: &'static TypeDesc, text: &str, f: &Failure, r: &Roots) -> u64 {
    let (line, col) = {
        let before = &text[..f.offset.min(text.len())];
        (before.matches('\n').count() + 1, before.len() - before.rfind('\n').map(|p| p + 1).unwrap_or(0) + 1)
    };
    let list_desc = err_desc.field("expected").unwrap().desc;
    let l = r.add(crate::list::new(list_desc));
    for e in &f.expected {
        let w = r.add(new_string(e));
        crate::list::push(l, w);
    }
    r.add(crate::make_record(
        err_desc,
        &[("offset", f.offset as u64), ("line", line as u64), ("column", col as u64), ("expected", l)],
    ))
}

/// `Result<Data, Failure>`: the shared packed parse forest.
pub extern "C" fn parse_forest(_c: *mut TaskCtx, _t: Targs, ret: *const TypeDesc, g: u64, input: u64) -> u64 {
    let ret = rdesc(ret);
    let grammar = GRAMMARS.lock().unwrap()[g as usize].clone();
    let text = str_of(input).to_string();
    let r = Roots::new();
    match grammar.forest_json(&text) {
        Ok(j) => {
            let d = r.add(crate::codec::data_of(ret.variant(0).unwrap().fields[0].desc, &j));
            crate::make_variant(ret, "ok", &[d])
        }
        Err(f) => {
            let v = failure_value(ret.variant(1).unwrap().fields[0].desc, &text, &f, &r);
            crate::make_variant(ret, "err", &[v])
        }
    }
}

/// `Result<{trees: List<Data>, truncated: Bool, cyclic: Bool}, Failure>`
pub extern "C" fn parse_trees(
    _c: *mut TaskCtx,
    _t: Targs,
    ret: *const TypeDesc,
    g: u64,
    input: u64,
    limit: u64,
) -> u64 {
    let ret = rdesc(ret);
    let grammar = GRAMMARS.lock().unwrap()[g as usize].clone();
    let text = str_of(input).to_string();
    let r = Roots::new();
    match grammar.trees(&text, limit as usize) {
        Ok((trees, truncated, cyclic)) => {
            let ok_desc = ret.variant(0).unwrap().fields[0].desc;
            let list_desc = ok_desc.field("trees").unwrap().desc;
            let data_desc = match &list_desc.kind {
                crate::desc::Kind::List { elem } => *elem,
                _ => list_desc,
            };
            let l = r.add(crate::list::new(list_desc));
            for t in &trees {
                let d = r.add(crate::codec::data_of(data_desc, &tree_json(t)));
                crate::list::push(l, d);
            }
            let v = r.add(crate::make_record(
                ok_desc,
                &[("trees", l), ("truncated", truncated as u64), ("cyclic", cyclic as u64)],
            ));
            crate::make_variant(ret, "ok", &[v])
        }
        Err(f) => {
            let v = failure_value(ret.variant(1).unwrap().fields[0].desc, &text, &f, &r);
            crate::make_variant(ret, "err", &[v])
        }
    }
}

/// `Result<{tree: Data, errors: List<Failure>}, Failure>`
pub extern "C" fn parse_recover(
    _c: *mut TaskCtx,
    _t: Targs,
    ret: *const TypeDesc,
    g: u64,
    input: u64,
    max_errors: u64,
) -> u64 {
    let ret = rdesc(ret);
    let grammar = GRAMMARS.lock().unwrap()[g as usize].clone();
    let text = str_of(input).to_string();
    let r = Roots::new();
    let err_desc = ret.variant(1).unwrap().fields[0].desc;
    match grammar.parse_recovering(&text, max_errors as usize) {
        Ok((tree, errors)) => {
            let ok_desc = ret.variant(0).unwrap().fields[0].desc;
            let d = r.add(crate::codec::data_of(ok_desc.field("tree").unwrap().desc, &tree_json(&tree)));
            let list_desc = ok_desc.field("errors").unwrap().desc;
            let l = r.add(crate::list::new(list_desc));
            for e in &errors {
                let v = failure_value(err_desc, &text, e, &r);
                crate::list::push(l, v);
            }
            let v = r.add(crate::make_record(ok_desc, &[("tree", d), ("errors", l)]));
            crate::make_variant(ret, "ok", &[v])
        }
        Err(f) => {
            let v = failure_value(err_desc, &text, &f, &r);
            crate::make_variant(ret, "err", &[v])
        }
    }
}

pub fn symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("lang_rt_i_parse_grammar", parse_grammar as *const u8),
        ("lang_rt_i_parse_run", parse_run as *const u8),
        ("lang_rt_i_parse_forest", parse_forest as *const u8),
        ("lang_rt_i_parse_trees", parse_trees as *const u8),
        ("lang_rt_i_parse_recover", parse_recover as *const u8),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_recursion_and_ambiguity() {
        let g = Grammar::parse_text("%skip /\\s+/\nexpr = expr \"+\" term | term\nterm = /[0-9]+/").unwrap();
        let (t, amb) = g.parse("1 + 2 + 3").map_err(|f| f.offset).unwrap();
        assert!(!amb);
        let Tree::Node { children, .. } = t else { panic!() };
        assert_eq!(children.len(), 3);
        let amb = Grammar::parse_text("e = e \"-\" e | /[0-9]/").unwrap();
        assert!(amb.parse("1-2-3").map_err(|f| f.offset).unwrap().1);
        let f = g.parse("1 + + 2").err().unwrap();
        assert_eq!(f.offset, 4);
        assert_eq!(f.expected, vec!["/[0-9]+/".to_string()]);
    }
}
