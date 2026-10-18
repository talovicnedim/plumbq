//! A small unified line diff, so the user sees exactly what an agent's file change would do.
//! Very large inputs are summarized instead.

use serde::{Deserialize, Serialize};

/// Lines of unchanged context around each change.
const CONTEXT: usize = 3;
/// Above this many lines on either side, only a summary is shown.
const MAX_LINES: usize = 2000;

/// One line of a change as the approval dialog shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    Added,
    Removed,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Keep,
    Remove,
    Add,
}

/// `old` → `new` as unified-diff hunks (`-` removed, `+` added, ` ` context), labelled with
/// `name`. Empty when nothing changed.
pub fn unified(name: &str, old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    if a.len() > MAX_LINES || b.len() > MAX_LINES {
        return format!(
            "--- {name}\n+++ {name}\n(too large to diff: {} lines before, {} after)\n",
            a.len(),
            b.len()
        );
    }
    let ops = edit_script(&a, &b);
    let mut out = format!("--- {name}\n+++ {name}\n");
    for hunk in hunks(&ops) {
        let (mut ia, mut ib) = (hunk.start_a, hunk.start_b);
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            ia + 1,
            hunk.len_a,
            ib + 1,
            hunk.len_b
        ));
        for op in &ops[hunk.from..hunk.to] {
            match op {
                Op::Keep => {
                    out.push_str(&format!(" {}\n", a[ia]));
                    ia += 1;
                    ib += 1;
                }
                Op::Remove => {
                    out.push_str(&format!("-{}\n", a[ia]));
                    ia += 1;
                }
                Op::Add => {
                    out.push_str(&format!("+{}\n", b[ib]));
                    ib += 1;
                }
            }
        }
    }
    out
}

/// Longest-common-subsequence edit script.
fn edit_script(a: &[&str], b: &[&str]) -> Vec<Op> {
    let (n, m) = (a.len(), b.len());
    // lcs[i][j]: LCS length of a[i..] and b[j..].
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut ops = Vec::with_capacity(n + m);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            ops.push(Op::Keep);
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
            // Removals first, like `diff -u`.
            ops.push(Op::Remove);
            i += 1;
        } else {
            ops.push(Op::Add);
            j += 1;
        }
    }
    ops
}

struct Hunk {
    from: usize,
    to: usize,
    start_a: usize,
    start_b: usize,
    len_a: usize,
    len_b: usize,
}

/// Groups changes with [`CONTEXT`] lines around them; nearby changes share a hunk.
fn hunks(ops: &[Op]) -> Vec<Hunk> {
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k] != Op::Keep).collect();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for &k in &changed {
        let from = k.saturating_sub(CONTEXT);
        let to = (k + CONTEXT + 1).min(ops.len());
        match ranges.last_mut() {
            Some(last) if from <= last.1 => last.1 = last.1.max(to),
            _ => ranges.push((from, to)),
        }
    }
    ranges
        .into_iter()
        .map(|(from, to)| {
            let before = &ops[..from];
            let start_a = before.iter().filter(|o| **o != Op::Add).count();
            let start_b = before.iter().filter(|o| **o != Op::Remove).count();
            let inside = &ops[from..to];
            Hunk {
                from,
                to,
                start_a,
                start_b,
                len_a: inside.iter().filter(|o| **o != Op::Add).count(),
                len_b: inside.iter().filter(|o| **o != Op::Remove).count(),
            }
        })
        .collect()
}

/// `text` split into lines, marking the added and removed ones of a [`unified`] diff. Other
/// text (a request as it would be sent) is all context.
pub fn lines(text: &str) -> Vec<DiffLine> {
    text.lines()
        .map(|line| {
            let kind = match line {
                l if l.starts_with('+') && !l.starts_with("+++") => DiffLineKind::Added,
                l if l.starts_with('-') && !l.starts_with("---") => DiffLineKind::Removed,
                _ => DiffLineKind::Context,
            };
            DiffLine {
                kind,
                text: line.to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_changed_lines_with_context() {
        let old = "### Get\nGET /a\nAccept: */*\n";
        let new = "### Get\nGET /b\nAccept: */*\nX-Id: 1\n";
        assert_eq!(
            unified("users/get.http", old, new),
            "--- users/get.http\n+++ users/get.http\n@@ -1,3 +1,4 @@\n ### Get\n-GET /a\n+GET /b\n \
             Accept: */*\n+X-Id: 1\n"
        );
    }

    #[test]
    fn new_and_deleted_files() {
        assert_eq!(
            unified("a.http", "", "GET /x\n"),
            "--- a.http\n+++ a.http\n@@ -1,0 +1,1 @@\n+GET /x\n"
        );
        assert!(unified("a.http", "GET /x\n", "").contains("-GET /x"));
        assert_eq!(unified("a.http", "same\n", "same\n"), "");
    }

    #[test]
    fn far_apart_changes_get_separate_hunks() {
        let old: String = (0..20).map(|i| format!("line {i}\n")).collect();
        let new = old
            .replace("line 1\n", "line one\n")
            .replace("line 18\n", "line eighteen\n");
        let diff = unified("f", &old, &new);
        assert_eq!(diff.matches("@@ -").count(), 2, "{diff}");
    }

    #[test]
    fn marks_added_and_removed_lines_but_not_file_names() {
        let kinds: Vec<DiffLineKind> = lines(&unified("a.http", "GET /a\n", "GET /b\n"))
            .into_iter()
            .map(|line| line.kind)
            .collect();
        assert!(kinds.contains(&DiffLineKind::Added));
        assert!(kinds.contains(&DiffLineKind::Removed));
        assert_eq!(kinds[0], DiffLineKind::Context);
    }
}
