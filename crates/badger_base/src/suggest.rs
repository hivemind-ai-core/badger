//! "Did you mean ...?": the known name nearest to a mistyped one.

/// Levenshtein distance (in characters).
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The candidate closest to `name`, within one edit for short names and
/// two for longer ones (ties go to the first in alphabetical order).
/// Names the compiler made up (`%...`) or marked unused (`_...`) are not
/// suggested.
pub fn nearest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let limit = if name.chars().count() <= 3 { 1 } else { 2 };
    let mut best: Option<(usize, &str)> = None;
    for n in candidates {
        if n == name || n.starts_with('%') || n.starts_with('_') {
            continue;
        }
        let d = edit_distance(name, n);
        if d <= limit && best.is_none_or(|(bd, bn)| d < bd || (d == bd && n < bn)) {
            best = Some((d, n));
        }
    }
    best.map(|(_, n)| n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_name() {
        assert_eq!(nearest("lenght", ["length", "first", "last"]), Some("length".into()));
        assert_eq!(nearest("nme", ["name", "id"]), Some("name".into()));
        assert_eq!(nearest("xyz", ["name", "id"]), None);
        assert_eq!(nearest("name", ["name"]), None);
    }
}
