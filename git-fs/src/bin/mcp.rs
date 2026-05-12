/// git-fs-mcp — MCP server over stdio. Set GIT_FS_REPO to the bare repo path.
use std::io::{BufRead, Write};

use anyhow::Context;
use git_fs::store::{MergeResult, Store};
use serde_json::{json, Value};

fn repo() -> String {
    std::env::var("GIT_FS_REPO").unwrap_or_else(|_| ".git-fs".to_string())
}

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Notifications (no id) don't get responses
        if msg.get("id").is_none() {
            continue;
        }

        let id = msg["id"].clone();
        let method = msg["method"].as_str().unwrap_or("");
        let params = &msg["params"];

        let response = match dispatch(method, params) {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err(e) => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}),
        };

        let _ = writeln!(out, "{}", serde_json::to_string(&response).unwrap());
        let _ = out.flush();
    }
}

fn dispatch(method: &str, params: &Value) -> anyhow::Result<Value> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "git-fs-mcp", "version": env!("CARGO_PKG_VERSION")}
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tool_list()})),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            let args = &params["arguments"];
            match call_tool(name, args) {
                Ok(text) => Ok(json!({"content":[{"type":"text","text":text}],"isError":false})),
                Err(e) => Ok(json!({"content":[{"type":"text","text":e.to_string()}],"isError":true})),
            }
        }
        _ => anyhow::bail!("unknown method: {method}"),
    }
}

// ── tool dispatcher ───────────────────────────────────────────────────────────

fn commit_json(oid: impl std::fmt::Display, branch: &str, path: &str) -> String {
    json!({"commit": oid.to_string(), "branch": branch, "path": path}).to_string()
}

fn call_tool(name: &str, a: &Value) -> anyhow::Result<String> {
    let r = repo();
    if name == "git_fs_init" {
        Store::init(&r)?;
        return Ok(format!("Initialized repo: {r}"));
    }
    let store = Store::open(&r)?;
    match name {
        "git_fs_branch_create" => {
            let name = a["name"].as_str().context("name required")?;
            let from = a["from"].as_str();
            store.branch_create(name, from)?;
            Ok(format!("Created branch: {name}"))
        }

        "git_fs_branch_list" => {
            let branches = store.branch_list()?;
            Ok(branches.join("\n"))
        }

        "git_fs_branch_delete" => {
            let name = a["name"].as_str().context("name required")?;
            store.branch_delete(name)?;
            Ok(format!("Deleted branch: {name}"))
        }

        "git_fs_write" => {
            let branch = a["branch"].as_str().context("branch required")?;
            let path = a["path"].as_str().context("path required")?;
            let content = a["content"].as_str().context("content required")?;
            let msg = a["message"].as_str().unwrap_or("write");
            let oid = store.write_file(branch, path, content.as_bytes(), msg)?;
            Ok(commit_json(oid, branch, path))
        }

        "git_fs_read" => {
            let reference = a["ref"].as_str().context("ref required")?;
            let path = a["path"].as_str().context("path required")?;
            let start_line = a["start_line"].as_u64().map(|n| n as usize);
            let end_line = a["end_line"].as_u64().map(|n| n as usize);
            if start_line.is_some() || end_line.is_some() {
                store.read_file_lines(reference, path, start_line, end_line)
            } else {
                let bytes = store.read_file(reference, path)?;
                String::from_utf8(bytes).context("file is not valid UTF-8")
            }
        }

        "git_fs_patch" => {
            let branch = a["branch"].as_str().context("branch required")?;
            let path = a["path"].as_str().context("path required")?;
            let start_line = a["start_line"].as_u64().context("start_line required")? as usize;
            let end_line = a["end_line"].as_u64().context("end_line required")? as usize;
            let content = a["content"].as_str().context("content required")?;
            let msg = a["message"].as_str().unwrap_or("patch");
            let oid = store.patch_file(branch, path, start_line, end_line, content, msg)?;
            Ok(commit_json(oid, branch, path))
        }

        "git_fs_replace" => {
            let branch = a["branch"].as_str().context("branch required")?;
            let path = a["path"].as_str().context("path required")?;
            let old_str = a["old_str"].as_str().context("old_str required")?;
            let new_str = a["new_str"].as_str().context("new_str required")?;
            let msg = a["message"].as_str().unwrap_or("replace");
            let oid = store.replace_file(branch, path, old_str, new_str, msg)?;
            Ok(commit_json(oid, branch, path))
        }

        "git_fs_rm" => {
            let branch = a["branch"].as_str().context("branch required")?;
            let path = a["path"].as_str().context("path required")?;
            let msg = match a["message"].as_str().filter(|s| !s.is_empty()) {
                Some(m) => m.to_string(),
                None => format!("rm {path}"),
            };
            let oid = store.remove_file(branch, path, &msg)?;
            Ok(json!({"commit": oid.to_string()}).to_string())
        }

        "git_fs_ls" => {
            let reference = a["ref"].as_str().context("ref required")?;
            let prefix = a["path"].as_str().unwrap_or("");
            let recursive = a["recursive"].as_bool().unwrap_or(false);
            let entries = store.list_files(reference, prefix, recursive)?;
            let v: Vec<Value> = entries
                .iter()
                .map(|e| {
                    json!({"path": e.path, "kind": e.kind, "size": e.size, "oid": e.oid})
                })
                .collect();
            Ok(serde_json::to_string_pretty(&v)?)
        }

        "git_fs_merge" => {
            let ours = a["ours"].as_str().context("ours required")?;
            let theirs = a["theirs"].as_str().context("theirs required")?;
            let base = a["base"].as_str().context("base required")?;
            let into = a["into"].as_str();
            let msg = a["message"].as_str().unwrap_or("merge");

            match store.merge(base, ours, theirs, into, msg)? {
                MergeResult::Clean { tree_oid, commit_oid } => {
                    Ok(json!({
                        "ok": true,
                        "tree": tree_oid.to_string(),
                        "commit": commit_oid.map(|o| o.to_string()),
                    })
                    .to_string())
                }
                MergeResult::Conflicts(conflicts) => {
                    let v: Vec<Value> = conflicts
                        .iter()
                        .map(|c| {
                            json!({
                                "path": c.path,
                                "ancestor": c.ancestor,
                                "ours": c.ours,
                                "theirs": c.theirs,
                            })
                        })
                        .collect();
                    anyhow::bail!(
                        "merge conflicts:\n{}",
                        serde_json::to_string_pretty(&v)?
                    )
                }
            }
        }

        "git_fs_diff" => {
            let ref_a = a["ref_a"].as_str().context("ref_a required")?;
            let ref_b = a["ref_b"].as_str().context("ref_b required")?;
            store.diff(ref_a, ref_b)
        }

        "git_fs_log" => {
            let reference = a["ref"].as_str().context("ref required")?;
            let count = a["count"].as_u64().unwrap_or(10) as usize;
            let entries = store.log(reference, count)?;
            let v: Vec<Value> = entries
                .iter()
                .map(|e| {
                    json!({"oid": e.oid, "message": e.message, "author": e.author, "time": e.time})
                })
                .collect();
            Ok(serde_json::to_string_pretty(&v)?)
        }

        "git_fs_checkout" => {
            let reference = a["ref"].as_str().context("ref required")?;
            let dest = a["dest"].as_str().context("dest required")?;
            store.checkout(reference, dest)?;
            Ok(format!("Checked out '{reference}' → {dest}"))
        }

        _ => anyhow::bail!("unknown tool: {name}"),
    }
}

// ── tool schemas ──────────────────────────────────────────────────────────────

fn tool_list() -> Value {
    json!([
        {
            "name": "git_fs_init",
            "description": "Initialize a bare git repo at the path in GITFS_REPO env var. Call once before anything else.",
            "inputSchema": {"type":"object","properties":{},"required":[]}
        },
        {
            "name": "git_fs_branch_create",
            "description": "Create a branch. Without `from`, creates an empty branch. With `from`, branches off that ref.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "name": {"type":"string","description":"New branch name"},
                    "from": {"type":"string","description":"Base ref to branch from (optional)"}
                },
                "required": ["name"]
            }
        },
        {
            "name": "git_fs_branch_list",
            "description": "List all branches in the repo.",
            "inputSchema": {"type":"object","properties":{},"required":[]}
        },
        {
            "name": "git_fs_branch_delete",
            "description": "Delete a branch.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "name": {"type":"string","description":"Branch to delete"}
                },
                "required": ["name"]
            }
        },
        {
            "name": "git_fs_write",
            "description": "Write text content to a file in a branch. Creates a commit. Intermediate directories are created automatically.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "branch":  {"type":"string","description":"Target branch"},
                    "path":    {"type":"string","description":"File path, e.g. src/main.rs"},
                    "content": {"type":"string","description":"Full file content"},
                    "message": {"type":"string","description":"Commit message (default: 'write')"}
                },
                "required": ["branch","path","content"]
            }
        },
        {
            "name": "git_fs_read",
            "description": "Read file content from a ref. Use start_line/end_line (1-indexed, inclusive) to read a slice instead of the full file.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "ref":        {"type":"string","description":"Git ref to read from"},
                    "path":       {"type":"string","description":"File path"},
                    "start_line": {"type":"integer","description":"First line to return (1-indexed, inclusive)"},
                    "end_line":   {"type":"integer","description":"Last line to return (1-indexed, inclusive)"}
                },
                "required": ["ref","path"]
            }
        },
        {
            "name": "git_fs_rm",
            "description": "Remove a file from a branch. Creates a commit.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "branch":  {"type":"string","description":"Target branch"},
                    "path":    {"type":"string","description":"File path to remove"},
                    "message": {"type":"string","description":"Commit message (optional)"}
                },
                "required": ["branch","path"]
            }
        },
        {
            "name": "git_fs_ls",
            "description": "List files in a ref. Use recursive=true to walk all subdirectories.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "ref":       {"type":"string","description":"Git ref"},
                    "path":      {"type":"string","description":"Subdirectory to list (optional, default: root)"},
                    "recursive": {"type":"boolean","description":"Walk subdirectories (default: false)"}
                },
                "required": ["ref"]
            }
        },
        {
            "name": "git_fs_merge",
            "description": "3-way merge two branches. On conflict, returns conflict details as error. On success with `into`, commits the merge.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "ours":    {"type":"string","description":"Our branch"},
                    "theirs":  {"type":"string","description":"Their branch"},
                    "base":    {"type":"string","description":"Common ancestor ref"},
                    "into":    {"type":"string","description":"Commit merge result to this branch (optional)"},
                    "message": {"type":"string","description":"Merge commit message (default: 'merge')"}
                },
                "required": ["ours","theirs","base"]
            }
        },
        {
            "name": "git_fs_diff",
            "description": "Unified diff between two refs.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "ref_a": {"type":"string","description":"First ref"},
                    "ref_b": {"type":"string","description":"Second ref"}
                },
                "required": ["ref_a","ref_b"]
            }
        },
        {
            "name": "git_fs_log",
            "description": "Commit log for a ref.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "ref":   {"type":"string","description":"Git ref"},
                    "count": {"type":"integer","description":"Max commits to return (default: 10)"}
                },
                "required": ["ref"]
            }
        },
        {
            "name": "git_fs_patch",
            "description": "Replace a line range in a file and commit. Equivalent to Edit but writes directly to git history. Lines are 1-indexed, inclusive.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "branch":     {"type":"string","description":"Target branch"},
                    "path":       {"type":"string","description":"File path"},
                    "start_line": {"type":"integer","description":"First line to replace (1-indexed, inclusive)"},
                    "end_line":   {"type":"integer","description":"Last line to replace (1-indexed, inclusive)"},
                    "content":    {"type":"string","description":"Replacement content for the specified line range"},
                    "message":    {"type":"string","description":"Commit message (default: 'patch')"}
                },
                "required": ["branch","path","start_line","end_line","content"]
            }
        },
        {
            "name": "git_fs_replace",
            "description": "Replace an exact string in a file and commit. old_str must match exactly once — include surrounding context to make it unique. Immune to line-number drift. Prefer over git_fs_patch for most edits.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "branch":  {"type":"string","description":"Target branch"},
                    "path":    {"type":"string","description":"File path"},
                    "old_str": {"type":"string","description":"Exact string to replace (must match exactly once)"},
                    "new_str": {"type":"string","description":"Replacement string"},
                    "message": {"type":"string","description":"Commit message (default: 'replace')"}
                },
                "required": ["branch","path","old_str","new_str"]
            }
        },
        {
            "name": "git_fs_checkout",
            "description": "Materialize a ref to disk. Final extraction step — writes all files to dest directory.",
            "inputSchema": {
                "type":"object",
                "properties": {
                    "ref":  {"type":"string","description":"Git ref to materialize"},
                    "dest": {"type":"string","description":"Destination directory path"}
                },
                "required": ["ref","dest"]
            }
        }
    ])
}
