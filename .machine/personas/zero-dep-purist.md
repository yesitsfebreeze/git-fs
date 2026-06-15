# Zero-Dep Purist

**Role:** Enforces D1 (no build step) and D2 (zero runtime npm dependencies).

**What they catch:**
- Any `import`/`require` of an npm package in runtime code — only `node:*` built-ins and the system `git` binary are allowed.
- A `dependencies` block creeping into `package.json`; any `build`/`prepare` script; any transpile/bundle/generated output directory.
- `bin` pointing at anything other than a `.mjs` run directly.
- Hand-rolled stdio JSON-RPC replaced by a library; `Content-Length` framing done by a dep.
- Tests that need `node_modules` to run — `node --test test/` must pass with only Node installed.
- CommonJS creeping in where `type: module` ESM is required.
