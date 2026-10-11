---
{
  "name": "git-fs",
  "description": "Git session branches for AI agents: guarded reads, writes, edits and search on refs/gitfs/<session>, materialized to the worktree on your terms.",
  "events": {
    "tool.gitfs.ls": {
      "description": "List a session branch, its owned paths and commits.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {
              "session": {
                "type": "string",
                "minLength": 1,
                "description": "Whose branch to list (ls); your own by default."
              }
            },
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.gitfs.diff": {
      "description": "Read the revision-bound diff of a session branch.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {
              "path": {
                "type": "string",
                "minLength": 1,
                "description": "Repo-relative path, no leading / or ..; absent answers the session-wide summary: every owned path with its status"
              },
              "offset": {
                "type": "integer",
                "minimum": 0
              },
              "limit": {
                "type": "integer",
                "minimum": 1,
                "maximum": 65536,
                "description": "Patch bytes per page (diff); below 128 a page still answers 128, so a page always moves past a multibyte character."
              },
              "expected_revision": {
                "type": "string",
                "description": "Required for diff continuation."
              }
            },
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.gitfs.rollback": {
      "description": "Undo one session commit or restore one path to its session-start content.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {
              "path": {
                "type": "string",
                "minLength": 1,
                "description": "Repo-relative path, no leading / or ..; absent answers the session-wide summary: every owned path with its status"
              },
              "commit": {
                "type": "string",
                "minLength": 1,
                "description": "A commit of your session to undo (rollback; or give `path` instead to put that path back to its content at session start)."
              }
            },
            "additionalProperties": false,
            "oneOf": [
              {
                "required": [
                  "commit"
                ],
                "not": {
                  "required": [
                    "path"
                  ]
                }
              },
              {
                "required": [
                  "path"
                ],
                "not": {
                  "required": [
                    "commit"
                  ]
                }
              }
            ]
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.gitfs.catchup": {
      "description": "Rebase the session onto current trunk after checking its owned paths.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {},
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.gitfs.materialize": {
      "description": "Materialize selected session paths into the worktree with overwrite guards.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {
              "paths": {
                "type": "array",
                "items": {
                  "type": "string"
                },
                "description": "Restrict to these owned paths (materialize, capture)."
              },
              "force": {
                "type": "boolean",
                "description": "Overwrite newer on-disk files (materialize)."
              }
            },
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.gitfs.capture": {
      "description": "Capture selected worktree paths into the session branch.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {
              "paths": {
                "type": "array",
                "items": {
                  "type": "string"
                },
                "description": "Restrict to these owned paths (materialize, capture)."
              }
            },
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.integrate.integrate": {
      "description": "Preview or commit the guarded session integration workflow.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {
              "message": {
                "type": "string",
                "description": "Reviewed commit subject; never bypasses a required gate."
              },
              "phase": {
                "type": "string",
                "enum": [
                  "preview",
                  "commit",
                  "reconcile"
                ]
              },
              "expected_revision": {
                "type": "string"
              },
              "force_secrets": {
                "type": "boolean",
                "description": "Integrate despite secrets in the diff."
              }
            },
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.integrate.push": {
      "description": "Preview, commit or reconcile a guarded push with reviewed revision evidence.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {
              "phase": {
                "type": "string",
                "enum": [
                  "preview",
                  "commit",
                  "reconcile"
                ]
              },
              "expected_revision": {
                "type": "string"
              },
              "remote": {
                "type": "string"
              },
              "ref": {
                "type": "string"
              },
              "commit": {
                "type": "string"
              },
              "expected_remote_head": {
                "type": [
                  "string",
                  "null"
                ]
              },
              "operation_id": {
                "type": "string"
              }
            },
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.integrate.undo": {
      "description": "Undo the last integrated session change.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {},
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    },
    "tool.integrate.scan": {
      "description": "Scan selected files for secrets.",
      "schema": {
        "type": "object",
        "required": [
          "op"
        ],
        "properties": {
          "op": {
            "type": "string",
            "enum": [
              "describe",
              "call",
              "cancel"
            ]
          },
          "input": {
            "type": "object",
            "required": [],
            "properties": {
              "paths": {
                "type": "array",
                "items": {
                  "type": "string"
                },
                "description": "Files to scan (scan)."
              }
            },
            "additionalProperties": false
          },
          "context": {
            "type": "object"
          }
        }
      }
    }
  },
  "config": {
    "mode": {
      "type": "string",
      "default": "git",
      "doc": "Where write and edit publish. git: one commit per change on refs/gitfs/<session>, the worktree untouched until gitfs_materialize; reads and searches prefer the session's changes, then committed trunk, then untracked disk files. Read responses identify their revision and disk divergence. local: write the worktree directly. A workspace outside a Git repository always behaves as local."
    },
    "max_read_bytes": {
      "type": "integer",
      "default": 8388608,
      "min": 128,
      "max": 268435456,
      "doc": "Bytes one read returns before it is truncated."
    },
    "max_read_lines": {
      "type": "integer",
      "default": 50000,
      "min": 1,
      "max": 5000000,
      "doc": "Lines one read returns before it is truncated."
    },
    "max_whole_read_lines": {
      "type": "integer",
      "default": 350,
      "min": 1,
      "max": 5000000,
      "doc": "Lines a file may have before a read with neither offset nor limit is refused and pointed at a range or a grep."
    },
    "max_file_bytes": {
      "type": "integer",
      "default": 268435456,
      "min": 1,
      "max": 4294967296,
      "doc": "Largest file git-fs will open at all."
    },
    "max_invocations": {
      "type": "integer",
      "default": 65536,
      "min": 1,
      "max": 4194304,
      "doc": "Invocations retained per run, for cancellation."
    },
    "max_runs": {
      "type": "integer",
      "default": 4096,
      "min": 1,
      "max": 1048576,
      "doc": "Runs retained alongside them."
    },
    "max_reads": {
      "type": "integer",
      "default": 65536,
      "min": 1,
      "max": 4194304,
      "doc": "Reads retained per run."
    },
    "max_search_items": {
      "type": "integer",
      "default": 50000,
      "min": 1,
      "max": 1000000,
      "doc": "Matches one search returns."
    },
    "max_search_bytes": {
      "type": "integer",
      "default": 16777216,
      "min": 1024,
      "max": 268435456,
      "doc": "Bytes of match text one search returns."
    },
    "max_search_wait_ms": {
      "type": "integer",
      "default": 120000,
      "min": 100,
      "max": 3600000,
      "doc": "How long a search may run before it is stopped."
    },
    "max_search_backend_bytes": {
      "type": "integer",
      "default": 268435456,
      "min": 1024,
      "max": 4294967296,
      "doc": "Bytes the search backend may produce."
    },
    "max_search_results_bytes": {
      "type": "integer",
      "default": 268435456,
      "min": 1024,
      "max": 4294967296,
      "doc": "Bytes of search results kept for paging through results."
    },
    "max_chains": {
      "type": "integer",
      "default": 4096,
      "min": 1,
      "max": 1048576,
      "doc": "Edit chains tracked at once."
    },
    "rg_program": {
      "type": "string",
      "default": "rg",
      "doc": "The ripgrep executable searches run, by basename or path."
    },
    "store_dir": {
      "type": "string",
      "default": ".git-fs",
      "doc": "Session overlay blobs and runtime state. Gitignored: it stays out of the tree it records."
    },
    "integrate.push": {
      "type": "boolean",
      "default": true,
      "doc": "Whether integrate publishes after previewing. It previews first and then publishes the exact reviewed tree locally; it does not enable implicit network access."
    },
    "integrate.remote": {
      "type": "string",
      "default": "origin",
      "doc": "The remote a push names."
    },
    "integrate.gate_model": {
      "type": "string",
      "default": null,
      "optional": true,
      "doc": "Model that gates a review. A configured gate requires an exact router grant and refuses on failure; without one the reviewed subject uses a plain file summary."
    },
    "integrate.gate_timeout_ms": {
      "type": "integer",
      "default": 300000,
      "min": 1,
      "max": 86400000,
      "doc": "How long that gate may take."
    },
    "integrate.push_timeout_ms": {
      "type": "integer",
      "default": 60000,
      "min": 100,
      "max": 60000,
      "doc": "How long a push may take."
    },
    "integrate.force_secrets": {
      "type": "boolean",
      "default": false,
      "doc": "Integrate even when the secret scan objects."
    },
    "integrate.author.name": {
      "type": "string",
      "default": "git-fs",
      "doc": "Author name recorded on an integrated commit."
    },
    "integrate.author.email": {
      "type": "string",
      "default": "git-fs@localhost",
      "doc": "Author email recorded on it."
    },
    "limits.provenance_bytes": {
      "type": "integer",
      "default": 268435456,
      "min": 1024,
      "max": 4294967296,
      "doc": "Bytes of provenance read from one source."
    },
    "limits.source_bytes": {
      "type": "integer",
      "default": 268435456,
      "min": 1024,
      "max": 4294967296,
      "doc": "Bytes of one source file inspected."
    },
    "limits.push_records": {
      "type": "integer",
      "default": 4096,
      "min": 1,
      "max": 1048576,
      "doc": "Push records kept."
    },
    "limits.push_record_bytes": {
      "type": "integer",
      "default": 262144,
      "min": 256,
      "max": 268435456,
      "doc": "Bytes one push record may take."
    },
    "limits.push_output_bytes": {
      "type": "integer",
      "default": 16777216,
      "min": 256,
      "max": 1073741824,
      "doc": "Bytes of push output carried back."
    }
  }
}
---

# git-fs manifest

The frontmatter above is the one place git-fs declares its configuration
defaults and bounds, and the contracts of the gitfs and integrate operations.
`src/service.rs` embeds this file and settles the configuration against it;
`GIT_FS_CONFIG` (a JSON object) lays overrides on top at startup.
