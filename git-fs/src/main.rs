use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::Read;

use git_fs::store::{MergeResult, Store};

#[derive(Parser)]
#[command(name = "git-fs", version, about = "Virtual filesystem over git object store — no working tree required")]
struct Cli {
    /// Path to bare git repository (or GITFS_REPO env var)
    #[arg(long, env = "GIT_FS_REPO", default_value = ".git-fs")]
    repo: String,

    /// Emit JSON instead of human-readable output
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Initialize a new bare git repository
    Init,

    /// Branch management
    #[command(subcommand)]
    Branch(BranchCmd),

    /// Write stdin to a file in a branch (auto-commits)
    Write {
        branch: String,
        path: String,
        #[arg(short, long, default_value = "write")]
        message: String,
    },

    /// Stream file content from a ref to stdout
    Read {
        #[arg(name = "ref")]
        reference: String,
        path: String,
    },

    /// Remove a file from a branch (auto-commits)
    Rm {
        branch: String,
        path: String,
        #[arg(short, long)]
        message: Option<String>,
    },

    /// List files in a ref
    Ls {
        #[arg(name = "ref")]
        reference: String,
        #[arg(default_value = "")]
        path: String,
        #[arg(short, long)]
        recursive: bool,
    },

    /// 3-way merge two branches (exit 2 on conflicts)
    Merge {
        ours: String,
        theirs: String,
        /// Common ancestor ref
        #[arg(long)]
        base: String,
        /// Commit merge result to this branch
        #[arg(long)]
        into: Option<String>,
        #[arg(short, long, default_value = "merge")]
        message: String,
    },

    /// Unified diff between two refs
    Diff {
        ref_a: String,
        ref_b: String,
    },

    /// Commit log for a ref
    Log {
        #[arg(name = "ref")]
        reference: String,
        #[arg(short, long, default_value = "10")]
        count: usize,
    },

    /// Materialize a ref to disk (final extraction step)
    Checkout {
        #[arg(name = "ref")]
        reference: String,
        dest: String,
    },

    /// Show commit info for a ref
    Show {
        #[arg(name = "ref")]
        reference: String,
    },

    /// Bootstrap a bare repo + write MCP server config to .mcp.json
    InitProject {
        /// Bare repo path (created if absent)
        #[arg(long, default_value = ".git-fs")]
        repo: String,
        /// MCP config file to update (default: .mcp.json in project root)
        #[arg(long, default_value = ".mcp.json")]
        mcp_config: String,
    },

    /// Delete stale agent branches. Defaults to a dry run.
    Prune {
        /// Only delete branches fully merged into this ref
        #[arg(long)]
        merged: bool,
        /// Only delete branches whose tip commit is older than this duration
        /// (e.g. `7d`, `24h`, `30m`). Combine with --merged for "safe sweep".
        #[arg(long, value_name = "DURATION")]
        older_than: Option<String>,
        /// Compare merged-status against this ref (default: main)
        #[arg(long, default_value = "main")]
        into: String,
        /// Branch-name prefix to consider (default: `agent/`)
        #[arg(long, default_value = "agent/")]
        prefix: String,
        /// Actually delete. Without this flag prune only reports.
        #[arg(long)]
        apply: bool,
    },

    /// Claude Code hook handlers — called directly by harness, cross-platform
    #[command(subcommand)]
    Hook(HookCmd),
}

#[derive(Subcommand)]
enum HookCmd {
    /// SessionStart: create agent branch from session_id + model in payload
    SessionStart,
    /// PostToolUse Write: read file from disk, commit to agent branch
    PostWrite,
    /// PostToolUse Edit: read file from disk, commit to agent branch
    PostEdit,
    /// PreToolUse Read: materialize file from git-fs branch to disk
    Read,
    /// Stop: auto-merge agent branch into main; write CONFLICTS.md on failure
    Stop,
}

#[derive(Subcommand)]
enum BranchCmd {
    /// Create branch (empty or from a ref)
    Create {
        name: String,
        #[arg(long)]
        from: Option<String>,
    },
    /// List all branches
    List,
    /// Delete a branch
    Delete { name: String },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let json = cli.json;

    match cli.command {
        Cmd::Init => {
            Store::init(&cli.repo)?;
            if json {
                println!("{}", serde_json::json!({"ok": true, "repo": cli.repo}));
            } else {
                println!("Initialized bare repo: {}", cli.repo);
            }
        }

        Cmd::Branch(action) => {
            let store = Store::open(&cli.repo)?;
            match action {
                BranchCmd::Create { name, from } => {
                    store.branch_create(&name, from.as_deref())?;
                    if json {
                        println!("{}", serde_json::json!({"ok": true, "branch": name}));
                    } else {
                        println!("Created branch: {name}");
                    }
                }
                BranchCmd::List => {
                    let branches = store.branch_list()?;
                    if json {
                        println!("{}", serde_json::json!(branches));
                    } else {
                        for b in &branches {
                            println!("{b}");
                        }
                    }
                }
                BranchCmd::Delete { name } => {
                    store.branch_delete(&name)?;
                    if json {
                        println!("{}", serde_json::json!({"ok": true, "deleted": name}));
                    } else {
                        println!("Deleted: {name}");
                    }
                }
            }
        }

        Cmd::Write {
            branch,
            path,
            message,
        } => {
            let store = Store::open(&cli.repo)?;
            let mut content = Vec::new();
            std::io::stdin().read_to_end(&mut content)?;
            let oid = store.write_file(&branch, &path, &content, &message)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "commit": oid.to_string(),
                        "branch": branch,
                        "path": path,
                    })
                );
            } else {
                println!("{} {}:{}", &oid.to_string()[..8], branch, path);
            }
        }

        Cmd::Read { reference, path } => {
            let store = Store::open(&cli.repo)?;
            let content = store.read_file(&reference, &path)?;
            std::io::Write::write_all(&mut std::io::stdout(), &content)?;
        }

        Cmd::Rm {
            branch,
            path,
            message,
        } => {
            let store = Store::open(&cli.repo)?;
            let msg = message.unwrap_or_else(|| format!("rm {path}"));
            let oid = store.remove_file(&branch, &path, &msg)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"ok": true, "commit": oid.to_string()})
                );
            } else {
                println!("{} rm {}", &oid.to_string()[..8], path);
            }
        }

        Cmd::Ls {
            reference,
            path,
            recursive,
        } => {
            let store = Store::open(&cli.repo)?;
            let entries = store.list_files(&reference, &path, recursive)?;
            if json {
                let v: Vec<_> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "path": e.path,
                            "kind": e.kind,
                            "size": e.size,
                            "oid":  e.oid,
                        })
                    })
                    .collect();
                println!("{}", serde_json::json!(v));
            } else {
                for e in &entries {
                    match e.kind {
                        "blob" => println!("{:>8}B  {}  {}", e.size, &e.oid[..8], e.path),
                        _ => println!("     dir  {}  {}/", &e.oid[..8], e.path),
                    }
                }
            }
        }

        Cmd::Merge {
            ours,
            theirs,
            base,
            into,
            message,
        } => {
            let store = Store::open(&cli.repo)?;
            let result = store.merge(&base, &ours, &theirs, into.as_deref(), &message)?;
            match result {
                MergeResult::Clean {
                    tree_oid,
                    commit_oid,
                } => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "ok": true,
                                "tree":   tree_oid.to_string(),
                                "commit": commit_oid.map(|o| o.to_string()),
                            })
                        );
                    } else {
                        match commit_oid {
                            Some(c) => println!(
                                "merged tree {} → commit {}",
                                &tree_oid.to_string()[..8],
                                &c.to_string()[..8]
                            ),
                            None => println!(
                                "tree {} (pass --into <branch> to commit)",
                                &tree_oid.to_string()[..8]
                            ),
                        }
                    }
                }
                MergeResult::Conflicts(conflicts) => {
                    if json {
                        let v: Vec<_> = conflicts
                            .iter()
                            .map(|c| {
                                serde_json::json!({
                                    "path":     c.path,
                                    "ancestor": c.ancestor,
                                    "ours":     c.ours,
                                    "theirs":   c.theirs,
                                })
                            })
                            .collect();
                        println!(
                            "{}",
                            serde_json::json!({"ok": false, "conflicts": v})
                        );
                    } else {
                        eprintln!("CONFLICTS ({}):", conflicts.len());
                        for c in &conflicts {
                            eprintln!("  conflict: {}", c.path);
                        }
                    }
                    std::process::exit(2);
                }
            }
        }

        Cmd::Diff { ref_a, ref_b } => {
            let store = Store::open(&cli.repo)?;
            let diff = store.diff(&ref_a, &ref_b)?;
            print!("{diff}");
        }

        Cmd::Log { reference, count } => {
            let store = Store::open(&cli.repo)?;
            let entries = store.log(&reference, count)?;
            if json {
                let v: Vec<_> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "oid":     e.oid,
                            "message": e.message,
                            "author":  e.author,
                            "time":    e.time,
                        })
                    })
                    .collect();
                println!("{}", serde_json::json!(v));
            } else {
                for e in &entries {
                    println!("{} {}", &e.oid[..8], e.message);
                }
            }
        }

        Cmd::Checkout { reference, dest } => {
            let store = Store::open(&cli.repo)?;
            store.checkout(&reference, &dest)?;
            if json {
                println!("{}", serde_json::json!({"ok": true, "dest": dest}));
            } else {
                println!("Checked out '{reference}' → {dest}");
            }
        }

        Cmd::Show { reference } => {
            let store = Store::open(&cli.repo)?;
            let entries = store.log(&reference, 1)?;
            if let Some(e) = entries.first() {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "oid":     e.oid,
                            "message": e.message,
                            "author":  e.author,
                            "time":    e.time,
                        })
                    );
                } else {
                    println!("commit {}", e.oid);
                    println!("author {}", e.author);
                    println!();
                    println!("    {}", e.message);
                }
            }
        }

        Cmd::InitProject { repo, mcp_config } => {
            // 1. Init or verify bare repo
            if std::path::Path::new(&repo).exists() {
                Store::open(&repo).map(|_| ()).or_else(|_| Store::init(&repo).map(|_| ()))?;
            } else {
                Store::init(&repo)?;
            }
            let repo_abs = {
                let p = std::fs::canonicalize(&repo)
                    .unwrap_or_else(|_| std::path::PathBuf::from(&repo));
                let s = p.to_string_lossy();
                let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
                std::path::PathBuf::from(&*s)
            };

            // 2. Read existing .mcp.json or start empty
            let cfg_path = std::path::Path::new(&mcp_config);
            let mut cfg: serde_json::Value = if cfg_path.exists() {
                let raw = std::fs::read_to_string(cfg_path)?;
                serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
            } else {
                serde_json::json!({})
            };

            // 3. Inject MCP server entry
            cfg["mcpServers"]["git-fs"] = serde_json::json!({
                "command": "git-fs-mcp",
                "env": { "GIT_FS_REPO": repo_abs.to_string_lossy() }
            });

            std::fs::write(cfg_path, serde_json::to_string_pretty(&cfg)?)?;

            println!("Repo:     {}", repo_abs.display());
            println!("Config:   {mcp_config}");
            println!();
            println!("Restart Claude Code to load the MCP server.");
            println!("Tools: git_fs_write, git_fs_read, git_fs_ls, git_fs_merge, git_fs_diff, git_fs_log ...");
        }

        Cmd::Prune {
            merged,
            older_than,
            into,
            prefix,
            apply,
        } => {
            let store = Store::open(&cli.repo)?;
            let cutoff = older_than
                .as_deref()
                .map(parse_duration_secs)
                .transpose()?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let mut report = Vec::new();
            for branch in store.branch_list()? {
                if !branch.starts_with(&prefix) || branch == into { continue; }
                let age = match store.tip_time(&branch) {
                    Ok(t) => now.saturating_sub(t),
                    Err(_) => continue,
                };
                if let Some(max) = cutoff {
                    if age < max { continue; }
                }
                let is_merged = store.is_merged_into(&branch, &into).unwrap_or(false);
                if merged && !is_merged { continue; }
                report.push((branch, age, is_merged));
            }

            let mut deleted = Vec::new();
            if apply {
                for (b, _, _) in &report {
                    match store.branch_delete(b) {
                        Ok(()) => deleted.push(b.clone()),
                        Err(e) => eprintln!("prune: failed to delete {b}: {e}"),
                    }
                }
            }

            if json {
                let v: Vec<_> = report
                    .iter()
                    .map(|(b, age, m)| {
                        serde_json::json!({
                            "branch": b,
                            "age_secs": age,
                            "merged": m,
                            "deleted": apply && deleted.contains(b),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({
                        "applied": apply,
                        "candidates": v,
                    })
                );
            } else if report.is_empty() {
                println!("nothing to prune");
            } else {
                let verb = if apply { "deleted" } else { "would delete" };
                println!("{verb} {} branch(es):", report.len());
                for (b, age, m) in &report {
                    let tag = if *m { "merged" } else { "UNMERGED" };
                    println!("  {b}  age={}s  {tag}", age);
                }
                if !apply {
                    println!("\n(dry run — pass --apply to delete)");
                }
            }
        }

        Cmd::Hook(action) => run_hook(action)?,
    }

    Ok(())
}

// ── hook helpers ──────────────────────────────────────────────────────────────

/// Translate absolute file path to repo-relative forward-slash path.
/// Returns None if path is not under cwd.
fn rel_path(abs: &str, cwd: &std::path::Path) -> Option<String> {
    let p = std::path::Path::new(abs);
    p.strip_prefix(cwd).ok().map(|r| r.to_string_lossy().replace('\\', "/"))
}

fn run_hook(action: HookCmd) -> Result<()> {
    let repo = std::env::var("GIT_FS_REPO").unwrap_or_else(|_| ".git-fs".to_string());
    let cwd  = std::env::current_dir()?;

    // All hooks silently fall through if repo missing — native tool proceeds unchanged
    if !std::path::Path::new(&repo).exists() {
        return Ok(());
    }

    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    let payload: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();

    let session_id = match payload["session_id"].as_str() { Some(s) => s, None => return Ok(()) };
    let branch = format!("agent/{session_id}");

    match action {
        HookCmd::SessionStart => {
            let model = payload["model"].as_str().unwrap_or("unknown");
            let store = Store::open(&repo)?;
            // Branch from main so agent inherits all project files; store base for stop hook
            let base_oid = store.resolve_commit_oid("main")
                .map(|oid| oid.to_string())
                .unwrap_or_default();
            let from = if base_oid.is_empty() { None } else { Some("main") };
            let _ = store.branch_create(&branch, from);
            store.write_file(
                &branch,
                ".agent",
                format!("model: {model}\nsession: {session_id}\nbase: {base_oid}\n").as_bytes(),
                &format!("session start\n\nmodel: {model}"),
            )?;
            // Seed session scratch (mergeignored — visible to siblings, stripped pre-merge)
            let intent_template = format!(
                "# Session intent\n\n\
                session: {session_id}\n\
                model: {model}\n\n\
                ## What this session is doing\n\n\n\
                ## Why\n\n\n\
                ## Paths touched\n\n(see .git-fs/session/paths.md)\n"
            );
            let _ = store.write_file(
                &branch,
                ".git-fs/session/intent.md",
                intent_template.as_bytes(),
                "seed session intent",
            );
            eprintln!("git-fs: branch '{branch}' ready (model: {model})");
            println!(
                "git-fs session active.\n\
                Branch: {branch}\n\
                All file changes are tracked automatically via hooks.\n\
                Prefer git_fs_replace for edits (text-match, immune to line-number drift).\n\
                Use git_fs_write for new files, git_fs_read to read, git_fs_patch for line-range edits.\n\
                Use git_fs_log branch:{branch} to see your change history.\n\
                Use git_fs_branch_list to see all agent sessions.\n\
                Load schemas upfront: ToolSearch select:git_fs_write,git_fs_read,git_fs_replace,git_fs_patch,git_fs_ls,git_fs_rm,git_fs_merge,git_fs_diff,git_fs_log,git_fs_branch_create,git_fs_branch_list,git_fs_branch_delete,git_fs_checkout"
            );
        }

        HookCmd::PostWrite | HookCmd::PostEdit => {
            let file_path = match payload["tool_input"]["file_path"].as_str() { Some(s) => s, None => return Ok(()) };
            let rel = match rel_path(file_path, &cwd) { Some(r) => r, None => return Ok(()) };
            if !std::path::Path::new(file_path).exists() { return Ok(()); }
            let content = std::fs::read(file_path)?;
            let op = match action { HookCmd::PostWrite => "write", _ => "edit" };
            let store = Store::open(&repo)?;
            if store.write_file(&branch, &rel, &content, &format!("{op} {rel}")).is_ok() {
                eprintln!("git-fs: {op} {branch}:{rel}");
            }
        }

        HookCmd::Read => {
            let file_path = match payload["tool_input"]["file_path"].as_str() { Some(s) => s, None => return Ok(()) };
            let rel = match rel_path(file_path, &cwd) { Some(r) => r, None => return Ok(()) };
            let store = Store::open(&repo)?;
            if let Ok(bytes) = store.read_file(&branch, &rel) {
                // Materialize git-fs content to disk so real Read sees it
                if let Some(parent) = std::path::Path::new(file_path).parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(file_path, &bytes)?;
            }
            // Exit 0 always — real Read proceeds from disk
        }

        HookCmd::Stop => {
            let store = Store::open(&repo)?;
            let branches = store.branch_list()?;
            if !branches.iter().any(|b| b == "main") {
                // No main yet — bootstrap: promote agent branch as main
                let agent_oid = store.resolve_commit_oid(&branch)?;
                store.repo.reference("refs/heads/main", agent_oid, true, "bootstrap main")?;
                let dest = cwd.to_str().unwrap_or(".");
                match store.checkout("main", dest) {
                    Ok(()) => eprintln!("git-fs: bootstrapped main from {branch}; materialized → {dest}"),
                    Err(e) => eprintln!("git-fs: checkout failed: {e}"),
                }
                return Ok(());
            }
            // Use base OID stored at session start for proper 3-way merge
            let base = store.read_file(&branch, ".agent").ok()
                .and_then(|b| String::from_utf8(b).ok())
                .and_then(|s| s.lines()
                    .find(|l| l.starts_with("base: "))
                    .map(|l| l[6..].trim().to_string()))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| branch.clone());

            // Strip mergeignored paths from agent branch BEFORE acquiring the merge
            // lock — strip only mutates the agent branch, no contention on main.
            let mi_set = load_mergeignore(&store, "main");
            match strip_mergeignored(&store, &branch, &mi_set) {
                Ok(0) => {}
                Ok(n) => eprintln!("git-fs: stripped {n} mergeignored path(s) from {branch}"),
                Err(e) => eprintln!("git-fs: strip mergeignored failed: {e}"),
            }

            // Sibling reconcile: warn when in-flight agent branches touch the
            // same paths we touched. Cheap, advisory — does not block the merge.
            if let Err(e) = report_sibling_overlap(&store, &branch, &mi_set) {
                eprintln!("git-fs: sibling reconcile failed: {e}");
            }

            // Serialize concurrent Stop hooks racing on main
            let _merge_lock = match acquire_merge_lock(&repo) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("git-fs: merge lock failed: {e}");
                    return Ok(());
                }
            };

            match store.merge(&base, &branch, "main", Some("main"),
                &format!("merge from {branch}")) {
                Ok(git_fs::store::MergeResult::Clean { commit_oid, .. }) => {
                    if commit_oid.is_some() {
                        eprintln!("git-fs: merged {branch} → main");
                    }
                    let dest = cwd.to_str().unwrap_or(".");
                    match store.checkout("main", dest) {
                        Ok(()) => eprintln!("git-fs: materialized main → {dest}"),
                        Err(e) => eprintln!("git-fs: checkout failed: {e}"),
                    }
                }
                Ok(git_fs::store::MergeResult::Conflicts(conflicts)) => {
                    let mut report = format!("# Conflicts: {branch} → main\n\n");
                    for c in &conflicts {
                        report.push_str(&format!("## {}\n\nours:\n```\n{}\n```\n\ntheirs:\n```\n{}\n```\n\n",
                            c.path,
                            c.ours.as_deref().unwrap_or("(deleted)"),
                            c.theirs.as_deref().unwrap_or("(deleted)"),
                        ));
                    }
                    let _ = store.write_file(&branch, "CONFLICTS.md", report.as_bytes(),
                        "conflict report");
                    eprintln!("git-fs: conflicts on merge {branch} → main; see CONFLICTS.md");
                }
                Err(e) => eprintln!("git-fs: merge error: {e}"),
            }
        }
    }

    Ok(())
}

// ── mergeignore + merge lock ─────────────────────────────────────────────────

/// Hard-coded mergeignore defaults — always applied, file or no file.
/// Per-session metadata that must never leak to main.
const HARD_MERGEIGNORE: &[&str] = &[".agent", "CONFLICTS.md"];

fn load_mergeignore(store: &Store, refname: &str) -> globset::GlobSet {
    let mut b = globset::GlobSetBuilder::new();
    for pat in HARD_MERGEIGNORE {
        if let Ok(g) = globset::Glob::new(pat) {
            b.add(g);
        }
    }
    if let Ok(bytes) = store.read_file(refname, ".git-fs/mergeignore") {
        let text = String::from_utf8_lossy(&bytes);
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            // gitignore-ish: trailing slash means dir → match everything under it
            let pat = if let Some(stripped) = line.strip_suffix('/') {
                format!("{stripped}/**")
            } else {
                line.to_string()
            };
            if let Ok(g) = globset::Glob::new(&pat) {
                b.add(g);
            }
        }
    }
    b.build().unwrap_or_else(|_| globset::GlobSet::empty())
}

fn strip_mergeignored(
    store: &Store,
    branch: &str,
    set: &globset::GlobSet,
) -> Result<usize> {
    if set.is_empty() { return Ok(0); }
    let entries = store.list_files(branch, "", true)?;
    let paths: Vec<String> = entries
        .into_iter()
        .filter(|e| e.kind == "blob" && set.is_match(&e.path))
        .map(|e| e.path)
        .collect();
    let n = paths.len();
    if n == 0 { return Ok(0); }
    store.remove_paths(
        branch,
        &paths,
        &format!("strip mergeignored ({n} path(s))"),
    )?;
    Ok(n)
}

/// Parse a short duration like `30s`, `15m`, `24h`, `7d` into seconds.
fn parse_duration_secs(s: &str) -> Result<i64> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("empty duration");
    }
    let (num, unit) = s.split_at(s.len() - 1);
    // Allow plain integer = seconds when no unit suffix.
    if unit.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return s.parse::<i64>().map_err(|e| anyhow::anyhow!("bad duration '{s}': {e}"));
    }
    let n: i64 = num
        .parse()
        .map_err(|e| anyhow::anyhow!("bad duration '{s}': {e}"))?;
    let secs = match unit {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86_400,
        "w" => n * 86_400 * 7,
        other => anyhow::bail!("unknown duration unit '{other}' in '{s}'"),
    };
    Ok(secs)
}

/// Walk all `agent/*` branches except `ours`, diff each against `main`, and
/// print a stderr warning for every path that overlaps with our own change-set.
/// Mergeignored paths are excluded since they never reach `main`.
fn report_sibling_overlap(
    store: &Store,
    ours: &str,
    mi_set: &globset::GlobSet,
) -> Result<()> {
    let our_paths: std::collections::BTreeSet<String> = store
        .changed_paths("main", ours)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !mi_set.is_match(p))
        .collect();
    if our_paths.is_empty() { return Ok(()); }

    let branches = store.branch_list()?;
    for sib in &branches {
        if sib == ours || !sib.starts_with("agent/") { continue; }
        let sib_paths = match store.changed_paths("main", sib) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let overlap: Vec<&String> = sib_paths
            .iter()
            .filter(|p| !mi_set.is_match(p) && our_paths.contains(*p))
            .collect();
        if overlap.is_empty() { continue; }
        eprintln!(
            "git-fs: sibling overlap with {sib} on {} path(s):",
            overlap.len()
        );
        for p in overlap { eprintln!("  {p}"); }
    }
    Ok(())
}

fn acquire_merge_lock(repo: &str) -> Result<std::fs::File> {
    use fs2::FileExt;
    let lock_path = std::path::Path::new(repo).join("merge.lock");
    let f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(&lock_path)?;
    f.lock_exclusive()?;
    Ok(f)
}
