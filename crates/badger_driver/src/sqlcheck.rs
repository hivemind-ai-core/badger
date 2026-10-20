//! Compile-time checks of literal SQL (`badger check`): statements are
//! prepared, not run, against a scratch in-memory database holding the
//! program's schema.

use rusqlite::Connection;

/// A result column: its name, declared type (none for an expression),
/// and whether it can be NULL (none when unknown).
pub struct Column {
    pub name: String,
    pub decl: Option<String>,
    pub nullable: Option<bool>,
}

pub struct Prepared {
    pub params: usize,
    /// Each parameter's name (`:id`), none for a positional `?`.
    pub names: Vec<Option<String>>,
    pub columns: Vec<Column>,
}

pub struct Db(Connection);

impl Db {
    pub fn new() -> Result<Db, String> {
        Connection::open_in_memory().map(Db).map_err(|e| e.to_string())
    }

    /// Run schema statements.
    pub fn run(&self, sql: &str) -> Result<(), String> {
        self.0.execute_batch(sql).map_err(|e| e.to_string())
    }

    pub fn prepare(&self, sql: &str) -> Result<Prepared, String> {
        let st = self.0.prepare(sql).map_err(|e| e.to_string())?;
        let decls: Vec<Option<String>> =
            st.columns().iter().map(|c| c.decl_type().map(str::to_string)).collect();
        let mut columns = vec![];
        for (i, name) in st.column_names().iter().enumerate() {
            let nullable = match st.column_metadata(i) {
                // an INTEGER PRIMARY KEY is the rowid, never NULL
                Ok(Some((_, _, _, ty, _, not_null, pk, _))) => {
                    let rowid = pk && ty.is_some_and(|t| t.to_string_lossy().eq_ignore_ascii_case("INTEGER"));
                    Some(!(not_null || rowid))
                }
                _ => None,
            };
            columns.push(Column { name: name.to_string(), decl: decls[i].clone(), nullable });
        }
        let names = (1..=st.parameter_count()).map(|i| st.parameter_name(i).map(str::to_string)).collect();
        Ok(Prepared { params: st.parameter_count(), names, columns })
    }
}

/// The statements of a batch (split at `;` outside quotes and comments;
/// empty statements dropped).
pub fn split(sql: &str) -> Vec<String> {
    let (mut out, mut cur) = (vec![], String::new());
    let cs: Vec<char> = sql.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            '\'' | '"' | '`' | '[' => {
                let close = if c == '[' { ']' } else { c };
                cur.push(c);
                i += 1;
                while i < cs.len() {
                    cur.push(cs[i]);
                    if cs[i] == close {
                        break;
                    }
                    i += 1;
                }
            }
            '-' if cs.get(i + 1) == Some(&'-') => {
                while i < cs.len() && cs[i] != '\n' {
                    i += 1;
                }
                cur.push('\n');
            }
            '/' if cs.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < cs.len() && !(cs[i] == '*' && cs[i + 1] == '/') {
                    i += 1;
                }
                i += 1;
                cur.push(' ');
            }
            ';' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
        i += 1;
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}
