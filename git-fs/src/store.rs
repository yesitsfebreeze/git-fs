use anyhow::{bail, Context, Result};
use git2::{BranchType, Odb, ObjectType, Oid, Repository, Signature, Tree};
use std::path::Path;

pub struct Store {
    pub repo: Repository,
}

// Raw git filemode constants
const MODE_BLOB: i32 = 0o100644;
const MODE_TREE: i32 = 0o040000;

impl Store {
    pub fn open(path: &str) -> Result<Self> {
        let repo = Repository::open_bare(path)
            .with_context(|| format!("Cannot open repo '{path}'. Run `git-fs init` first."))?;
        Ok(Self { repo })
    }

    pub fn init(path: &str) -> Result<Self> {
        let repo = Repository::init_bare(path)?;
        Ok(Self { repo })
    }

    fn branch_ref(branch: &str) -> String {
        if branch.starts_with("refs/") {
            branch.to_string()
        } else {
            format!("refs/heads/{branch}")
        }
    }

    fn sig() -> Result<Signature<'static>> {
        Ok(Signature::now("git-fs", "git-fs@local")?)
    }

    pub fn resolve_commit_oid(&self, refname: &str) -> Result<Oid> {
        self.repo
            .revparse_single(refname)
            .with_context(|| format!("Cannot resolve '{refname}'"))?
            .peel_to_commit()
            .map(|c| c.id())
            .map_err(Into::into)
    }

    pub fn resolve_tree<'a>(&'a self, refname: &str) -> Result<Tree<'a>> {
        let oid = self.resolve_commit_oid(refname)?;
        let commit = self.repo.find_commit(oid)?;
        Ok(commit.tree()?)
    }

    // ── branch ───────────────────────────────────────────────────────────────

    pub fn branch_create(&self, name: &str, from: Option<&str>) -> Result<()> {
        let refname = Self::branch_ref(name);
        if let Some(from_ref) = from {
            let oid = self.resolve_commit_oid(from_ref)?;
            let commit = self.repo.find_commit(oid)?;
            self.repo.branch(name, &commit, false)?;
        } else {
            let sig = Self::sig()?;
            let tree_oid = self.repo.treebuilder(None)?.write()?;
            let tree = self.repo.find_tree(tree_oid)?;
            self.repo
                .commit(Some(&refname), &sig, &sig, "init", &tree, &[])?;
        }
        Ok(())
    }

    pub fn branch_list(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for b in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = b?;
            if let Some(name) = branch.name()? {
                out.push(name.to_string());
            }
        }
        Ok(out)
    }

    pub fn branch_delete(&self, name: &str) -> Result<()> {
        self.repo
            .find_branch(name, BranchType::Local)?
            .delete()?;
        Ok(())
    }

    // ── file ops ─────────────────────────────────────────────────────────────

    pub fn write_file(
        &self,
        branch: &str,
        path: &str,
        content: &[u8],
        msg: &str,
    ) -> Result<Oid> {
        let blob_oid = self.repo.blob(content)?;
        let refname = Self::branch_ref(branch);

        let parent = self
            .repo
            .find_reference(&refname)
            .ok()
            .and_then(|r| r.peel_to_commit().ok());

        let base_tree = parent.as_ref().map(|c| c.tree()).transpose()?;
        let new_tree_oid = self.tree_insert(base_tree.as_ref(), path, blob_oid)?;
        let new_tree = self.repo.find_tree(new_tree_oid)?;

        let sig = Self::sig()?;
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        Ok(self
            .repo
            .commit(Some(&refname), &sig, &sig, msg, &new_tree, &parents)?)
    }

    pub fn remove_file(&self, branch: &str, path: &str, msg: &str) -> Result<Oid> {
        let refname = Self::branch_ref(branch);
        let parent = self
            .repo
            .find_reference(&refname)?
            .peel_to_commit()?;
        let base_tree = parent.tree()?;
        let new_tree_oid = self.tree_remove(Some(&base_tree), path)?;
        let new_tree = self.repo.find_tree(new_tree_oid)?;
        let sig = Self::sig()?;
        Ok(self
            .repo
            .commit(Some(&refname), &sig, &sig, msg, &new_tree, &[&parent])?)
    }

    /// Remove many paths in a single commit. Missing paths are skipped silently.
    /// Returns the new commit OID, or None when `paths` is empty.
    pub fn remove_paths(&self, branch: &str, paths: &[String], msg: &str) -> Result<Option<Oid>> {
        if paths.is_empty() {
            return Ok(None);
        }
        let refname = Self::branch_ref(branch);
        let parent = self.repo.find_reference(&refname)?.peel_to_commit()?;
        let base_tree = parent.tree()?;
        let mut tree_oid = base_tree.id();
        let mut removed = 0usize;
        for path in paths {
            let cur = self.repo.find_tree(tree_oid)?;
            match self.tree_remove(Some(&cur), path) {
                Ok(oid) => {
                    tree_oid = oid;
                    removed += 1;
                }
                Err(_) => continue, // missing path → skip
            }
        }
        if removed == 0 {
            return Ok(None);
        }
        let new_tree = self.repo.find_tree(tree_oid)?;
        let sig = Self::sig()?;
        Ok(Some(self.repo.commit(
            Some(&refname),
            &sig,
            &sig,
            msg,
            &new_tree,
            &[&parent],
        )?))
    }

    pub fn read_file(&self, refname: &str, path: &str) -> Result<Vec<u8>> {
        let tree = self.resolve_tree(refname)?;
        let entry = tree
            .get_path(Path::new(path))
            .with_context(|| format!("'{path}' not found in '{refname}'"))?;
        let blob = self.repo.find_blob(entry.id())?;
        Ok(blob.content().to_vec())
    }

    fn read_text(&self, refname: &str, path: &str) -> Result<String> {
        let raw = self.read_file(refname, path)?;
        String::from_utf8(raw).with_context(|| format!("'{path}' is not valid UTF-8"))
    }

    /// Read a line range (1-indexed, inclusive). Returns UTF-8 slice.
    pub fn read_file_lines(
        &self,
        refname: &str,
        path: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<String> {
        let text = self.read_text(refname, path)?;
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();
        let start = start_line.unwrap_or(1).saturating_sub(1);
        let end = end_line.unwrap_or(total).min(total);
        if start >= end && total > 0 {
            anyhow::bail!("start_line {start_line:?} >= end_line {end_line:?}");
        }
        Ok(lines[start..end].join("\n"))
    }

    /// Replace lines [start_line..=end_line] (1-indexed, inclusive) with new_content and commit.
    pub fn patch_file(
        &self,
        branch: &str,
        path: &str,
        start_line: usize,
        end_line: usize,
        new_content: &str,
        msg: &str,
    ) -> Result<Oid> {
        let text = self.read_text(branch, path)?;
        let had_trailing_newline = text.ends_with('\n');
        let mut lines: Vec<&str> = text.lines().collect();
        let start = start_line.saturating_sub(1);
        let end = end_line.min(lines.len());
        anyhow::ensure!(start <= end, "start_line {start_line} > end_line {end_line}");
        let replacement: Vec<&str> = new_content.lines().collect();
        lines.splice(start..end, replacement);
        let mut result = lines.join("\n");
        if had_trailing_newline {
            result.push('\n');
        }
        self.write_file(branch, path, result.as_bytes(), msg)
    }

    /// Replace an exact string in a file and commit. old_str must match exactly once.
    pub fn replace_file(
        &self,
        branch: &str,
        path: &str,
        old_str: &str,
        new_str: &str,
        msg: &str,
    ) -> Result<Oid> {
        let text = self.read_text(branch, path)?;
        let count = text.matches(old_str).count();
        anyhow::ensure!(count > 0, "old_str not found in '{path}'");
        anyhow::ensure!(count == 1, "old_str matches {count} locations in '{path}'; make it unique by including more context");
        let result = text.replacen(old_str, new_str, 1);
        self.write_file(branch, path, result.as_bytes(), msg)
    }

    pub fn list_files(
        &self,
        refname: &str,
        prefix: &str,
        recursive: bool,
    ) -> Result<Vec<ListEntry>> {
        let root = self.resolve_tree(refname)?;

        // Navigate into prefix if given, using OID to avoid lifetime issues
        let tree_oid = if prefix.is_empty() {
            root.id()
        } else {
            root.get_path(Path::new(prefix))
                .with_context(|| format!("Path '{prefix}' not found in '{refname}'"))?
                .id()
        };

        let tree = self.repo.find_tree(tree_oid)?;
        let mut out = Vec::new();
        let odb = self.repo.odb()?;
        self.walk_tree(&odb, &tree, prefix, recursive, &mut out)?;
        Ok(out)
    }

    // ── merge ────────────────────────────────────────────────────────────────

    pub fn merge(
        &self,
        base_ref: &str,
        ours_ref: &str,
        theirs_ref: &str,
        target_branch: Option<&str>,
        msg: &str,
    ) -> Result<MergeResult> {
        let base_tree = self.resolve_tree(base_ref)?;
        let our_tree = self.resolve_tree(ours_ref)?;
        let their_tree = self.resolve_tree(theirs_ref)?;

        let mut idx =
            self.repo
                .merge_trees(&base_tree, &our_tree, &their_tree, None)?;

        if idx.has_conflicts() {
            let mut conflicts = Vec::new();
            for entry in idx.conflicts()? {
                let c = entry?;
                let path = c
                    .our
                    .as_ref()
                    .or(c.their.as_ref())
                    .map(|e| String::from_utf8_lossy(&e.path).into_owned())
                    .unwrap_or_default();

                let read_blob = |e: &git2::IndexEntry| -> Option<String> {
                    if e.id.is_zero() {
                        return None;
                    }
                    self.repo
                        .find_blob(e.id)
                        .ok()
                        .map(|b| String::from_utf8_lossy(b.content()).into_owned())
                };

                conflicts.push(ConflictEntry {
                    path,
                    ancestor: c.ancestor.as_ref().and_then(read_blob),
                    ours: c.our.as_ref().and_then(read_blob),
                    theirs: c.their.as_ref().and_then(read_blob),
                });
            }
            return Ok(MergeResult::Conflicts(conflicts));
        }

        let tree_oid = idx.write_tree_to(&self.repo)?;

        let commit_oid = if let Some(target) = target_branch {
            let our_commit =
                self.repo.find_commit(self.resolve_commit_oid(ours_ref)?)?;
            let their_commit =
                self.repo.find_commit(self.resolve_commit_oid(theirs_ref)?)?;
            let tree = self.repo.find_tree(tree_oid)?;
            let sig = Self::sig()?;
            let refname = Self::branch_ref(target);
            // target's current tip must be first parent for the ref update to succeed
            let target_tip = self.resolve_commit_oid(target).ok();
            let parents: Vec<&git2::Commit> = if target_tip == Some(their_commit.id()) {
                vec![&their_commit, &our_commit]
            } else {
                vec![&our_commit, &their_commit]
            };
            Some(self.repo.commit(Some(&refname), &sig, &sig, msg, &tree, &parents)?)
        } else {
            None
        };

        Ok(MergeResult::Clean {
            tree_oid,
            commit_oid,
        })
    }

    // ── diff ─────────────────────────────────────────────────────────────────

    pub fn diff(&self, ref_a: &str, ref_b: &str) -> Result<String> {
        let tree_a = self.resolve_tree(ref_a)?;
        let tree_b = self.resolve_tree(ref_b)?;
        let diff =
            self.repo
                .diff_tree_to_tree(Some(&tree_a), Some(&tree_b), None)?;

        let mut out = String::new();
        diff.print(git2::DiffFormat::Patch, |_d, _h, line| {
            let origin = line.origin();
            if let Ok(s) = std::str::from_utf8(line.content()) {
                match origin {
                    '+' | '-' | ' ' => {
                        out.push(origin);
                        out.push_str(s);
                    }
                    _ => out.push_str(s),
                }
            }
            true
        })?;

        Ok(out)
    }

    // ── log ──────────────────────────────────────────────────────────────────

    pub fn log(&self, refname: &str, count: usize) -> Result<Vec<LogEntry>> {
        let start = self.resolve_commit_oid(refname)?;
        let mut walk = self.repo.revwalk()?;
        walk.push(start)?;
        walk.set_sorting(git2::Sort::TIME)?;

        let mut out = Vec::new();
        for (i, oid) in walk.enumerate() {
            if i >= count {
                break;
            }
            let oid = oid?;
            let c = self.repo.find_commit(oid)?;
            out.push(LogEntry {
                oid: oid.to_string(),
                message: c.message().unwrap_or("").trim().to_string(),
                author: c.author().name().unwrap_or("").to_string(),
                time: c.time().seconds(),
            });
        }
        Ok(out)
    }

    // ── checkout ─────────────────────────────────────────────────────────────

    pub fn checkout(&self, refname: &str, dest: &str) -> Result<()> {
        let tree = self.resolve_tree(refname)?;
        let dest_path = Path::new(dest);
        std::fs::create_dir_all(dest_path)?;
        self.extract_tree(&tree, dest_path)
    }

    fn extract_tree(&self, tree: &Tree, dest: &Path) -> Result<()> {
        for entry in tree.iter() {
            let name = entry.name().unwrap_or("");
            let path = dest.join(name);
            match entry.kind() {
                Some(ObjectType::Blob) => {
                    let blob = self.repo.find_blob(entry.id())?;
                    std::fs::write(&path, blob.content())?;
                }
                Some(ObjectType::Tree) => {
                    std::fs::create_dir_all(&path)?;
                    let sub = self.repo.find_tree(entry.id())?;
                    self.extract_tree(&sub, &path)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    // ── tree helpers ─────────────────────────────────────────────────────────

    fn tree_insert(&self, base: Option<&Tree>, path: &str, blob: Oid) -> Result<Oid> {
        let mut parts = path.splitn(2, '/');
        let head = parts.next().unwrap();
        let tail = parts.next();
        let mut b = self.repo.treebuilder(base)?;
        match tail {
            None => {
                b.insert(head, blob, MODE_BLOB)?;
            }
            Some(rest) => {
                let sub_base = b.get(head)?.and_then(|e| self.repo.find_tree(e.id()).ok());
                let sub_oid = self.tree_insert(sub_base.as_ref(), rest, blob)?;
                b.insert(head, sub_oid, MODE_TREE)?;
            }
        }
        Ok(b.write()?)
    }

    fn tree_remove(&self, base: Option<&Tree>, path: &str) -> Result<Oid> {
        let mut parts = path.splitn(2, '/');
        let head = parts.next().unwrap();
        let tail = parts.next();
        let mut b = self.repo.treebuilder(base)?;
        match tail {
            None => {
                if b.get(head)?.is_none() {
                    bail!("File '{path}' not found");
                }
                b.remove(head)?;
            }
            Some(rest) => {
                let sub_base = b.get(head)?.and_then(|e| self.repo.find_tree(e.id()).ok());
                let sub_oid = self.tree_remove(sub_base.as_ref(), rest)?;
                b.insert(head, sub_oid, MODE_TREE)?;
            }
        }
        Ok(b.write()?)
    }

    fn walk_tree(
        &self,
        odb: &Odb,
        tree: &Tree,
        prefix: &str,
        recursive: bool,
        out: &mut Vec<ListEntry>,
    ) -> Result<()> {
        for entry in tree.iter() {
            let name = entry.name().unwrap_or("");
            let full = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}/{name}")
            };
            match entry.kind() {
                Some(ObjectType::Blob) => {
                    let size = odb.read_header(entry.id()).map(|(s, _)| s).unwrap_or(0);
                    out.push(ListEntry {
                        path: full,
                        kind: "blob",
                        size,
                        oid: entry.id().to_string(),
                    });
                }
                Some(ObjectType::Tree) => {
                    if recursive {
                        let sub = self.repo.find_tree(entry.id())?;
                        self.walk_tree(odb, &sub, &full, recursive, out)?;
                    } else {
                        out.push(ListEntry {
                            path: full,
                            kind: "tree",
                            size: 0,
                            oid: entry.id().to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

// ── public output types ───────────────────────────────────────────────────────

pub enum MergeResult {
    Clean {
        tree_oid: Oid,
        commit_oid: Option<Oid>,
    },
    Conflicts(Vec<ConflictEntry>),
}

pub struct ConflictEntry {
    pub path: String,
    pub ancestor: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
}

pub struct ListEntry {
    pub path: String,
    pub kind: &'static str,
    pub size: usize,
    pub oid: String,
}

pub struct LogEntry {
    pub oid: String,
    pub message: String,
    pub author: String,
    pub time: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::init(dir.path().to_str().unwrap()).unwrap();
        (dir, store)
    }

    #[test]
    fn write_and_read_file() {
        let (_dir, store) = temp_store();
        store.branch_create("main", None).unwrap();
        store.write_file("main", "hello.txt", b"world", "add hello").unwrap();
        let content = store.read_file("main", "hello.txt").unwrap();
        assert_eq!(content, b"world");
    }

    #[test]
    fn write_nested_path() {
        let (_dir, store) = temp_store();
        store.branch_create("main", None).unwrap();
        store.write_file("main", "src/lib.rs", b"pub fn f() {}", "add lib").unwrap();
        let content = store.read_file("main", "src/lib.rs").unwrap();
        assert_eq!(content, b"pub fn f() {}");
    }

    #[test]
    fn branch_create_list_delete() {
        let (_dir, store) = temp_store();
        store.branch_create("alpha", None).unwrap();
        store.branch_create("beta", None).unwrap();
        let branches = store.branch_list().unwrap();
        assert!(branches.contains(&"alpha".to_string()));
        assert!(branches.contains(&"beta".to_string()));
        store.branch_delete("alpha").unwrap();
        let branches = store.branch_list().unwrap();
        assert!(!branches.contains(&"alpha".to_string()));
    }

    #[test]
    fn list_files_flat_and_recursive() {
        let (_dir, store) = temp_store();
        store.branch_create("main", None).unwrap();
        store.write_file("main", "a.txt", b"a", "a").unwrap();
        store.write_file("main", "sub/b.txt", b"b", "b").unwrap();

        let flat = store.list_files("main", "", false).unwrap();
        let flat_names: Vec<&str> = flat.iter().map(|e| e.path.as_str()).collect();
        assert!(flat_names.contains(&"a.txt"));
        assert!(flat_names.contains(&"sub"));

        let rec = store.list_files("main", "", true).unwrap();
        let rec_names: Vec<&str> = rec.iter().map(|e| e.path.as_str()).collect();
        assert!(rec_names.contains(&"a.txt"));
        assert!(rec_names.contains(&"sub/b.txt"));
    }

    #[test]
    fn remove_file() {
        let (_dir, store) = temp_store();
        store.branch_create("main", None).unwrap();
        store.write_file("main", "bye.txt", b"x", "add").unwrap();
        store.remove_file("main", "bye.txt", "rm bye").unwrap();
        assert!(store.read_file("main", "bye.txt").is_err());
    }

    #[test]
    fn log_entries() {
        let (_dir, store) = temp_store();
        store.branch_create("main", None).unwrap();
        store.write_file("main", "f1.txt", b"1", "first commit").unwrap();
        store.write_file("main", "f2.txt", b"2", "second commit").unwrap();
        let entries = store.log("main", 10).unwrap();
        let msgs: Vec<&str> = entries.iter().map(|e| e.message.as_str()).collect();
        assert!(msgs.contains(&"first commit"));
        assert!(msgs.contains(&"second commit"));
        assert_eq!(entries[0].oid.len(), 40);
    }

    #[test]
    fn diff_shows_changes() {
        let (_dir, store) = temp_store();
        store.branch_create("main", None).unwrap();
        store.write_file("main", "x.txt", b"hello\n", "v1").unwrap();
        let v1 = store.log("main", 1).unwrap().into_iter().next().unwrap().oid;
        store.write_file("main", "x.txt", b"hello\nworld\n", "v2").unwrap();
        let diff = store.diff(&v1, "main").unwrap();
        assert!(diff.contains("+world"));
    }
}
