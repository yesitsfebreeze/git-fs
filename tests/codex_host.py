"""Exercise the installed Codex runtime with deterministic local model replies.

No API key, network model, or user thread. Requires codex and a release build.
Run: python3 tests/codex_host.py [--code-mode]
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SOURCE = Path(__file__).resolve().parents[1]
CODE_MODE = '--code-mode' in sys.argv
REQUESTS = []
ERRORS = []


def decoded_objects(value):
    """Unwrap tool JSON, including the extra text envelope in code mode."""
    if isinstance(value, str):
        try:
            yield from decoded_objects(json.loads(value))
        except (ValueError, TypeError):
            pass
    elif isinstance(value, dict):
        yield value
        for child in value.values():
            yield from decoded_objects(child)
    elif isinstance(value, list):
        for child in value:
            yield from decoded_objects(child)


def response_item(step, body):
    patch = '*** Begin Patch\n*** Update File: a.txt\n@@\n-base\n+bypass\n*** End Patch'
    actions = [
        ('apply_patch', patch),
        ('mcp__git-fs__batch', {'requests': [
            {'tool': 'read', 'arguments': {'path': 'a.txt'}},
            {'tool': 'glob', 'arguments': {'pattern': '*.txt'}},
        ]}),
        ('mcp__git-fs__write', {'path': 'a.txt', 'text': 'overlay\n'}),
        ('exec_command', {'cmd': 'cat a.txt', 'max_output_tokens': 100}),
        ('mcp__git-fs__read', {'path': 'a.txt'}),
    ]
    if step >= len(actions):
        return {'type': 'message', 'id': 'final', 'role': 'assistant',
                'content': [{'type': 'output_text', 'text': 'HOST_CHECK_DONE'}]}
    name, args = actions[step]
    if CODE_MODE:
        if name.startswith('mcp__'):
            suffix = name.rsplit('__', 1)[1]
            code = ('const t = ALL_TOOLS.find(t => t.name.includes("git") && '
                    f't.name.endsWith("__{suffix}")); text(await tools[t.name]({json.dumps(args)}));')
        else:
            code = f'text(await tools.{name}({json.dumps(args)}));'
        return {'type': 'custom_tool_call', 'call_id': f'c{step}', 'name': 'exec', 'input': code}
    if isinstance(args, str):
        return {'type': 'custom_tool_call', 'call_id': f'c{step}', 'name': name, 'input': args}
    return {'type': 'function_call', 'call_id': f'c{step}', 'name': name.rsplit('__', 1)[-1] if name.startswith('mcp__') else name, **({'namespace': 'mcp__git_fs'} if name.startswith('mcp__') else {}), 'arguments': json.dumps(args)}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        try:
            body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            step = len(REQUESTS)
            REQUESTS.append(body)
            item = response_item(step, body)
            events = [
                {'type': 'response.created', 'response': {'id': f'r{step}'}},
                {'type': 'response.output_item.done', 'output_index': 0, 'item': item},
                {'type': 'response.completed', 'response': {'id': f'r{step}', 'output': [item],
                 'usage': {'input_tokens': 100, 'output_tokens': 10, 'total_tokens': 110}}},
            ]
            data = ''.join(f'event: {e["type"]}\ndata: {json.dumps(e)}\n\n' for e in events).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except Exception as error:
            ERRORS.append(str(error))
            self.send_error(500)


def main():
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        with tempfile.TemporaryDirectory(prefix='gitfs-host-') as tmp:
            base = Path(tmp).resolve()
            config_home = base / 'codex'
            config_home.mkdir()
            workspace = base / 'repo'
            workspace.mkdir()
            env = dict(os.environ, CODEX_HOME=str(config_home))
            for key in ['GIT_FS_CONFIG', 'GIT_FS_SESSION', 'GIT_FS_DISK']:
                env.pop(key, None)
            (config_home / 'config.toml').write_text(f'''model = "gpt-6.1-sol"
model_provider = "fixture"
[features]
hooks = true
code_mode = {str(CODE_MODE).lower()}
[model_providers.fixture]
name = "Fixture"
base_url = "http://127.0.0.1:{server.server_port}/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
''')
            def run(args, **kwargs):
                return subprocess.run(args, env=env, cwd=workspace, capture_output=True, text=True, check=True, timeout=90, **kwargs)
            for args in [['init', '-q'], ['config', 'user.name', 'Test'], ['config', 'user.email', 'test@example.com']]:
                run(['git', *args])
            (workspace / 'a.txt').write_text('base\n')
            run(['git', 'add', '.'])
            run(['git', 'commit', '-qm', 'base'])
            run(['codex', 'plugin', 'marketplace', 'add', str(SOURCE)])
            run(['codex', 'plugin', 'add', 'git-fs@git-fs'])
            # Vetted fixture hooks only, in a disposable config home. No persisted
            # trust or security settings in the user's installation are changed.
            result = run(['codex', '--no-daemon', '--dangerously-bypass-hook-trust',
                          'exec', '--json', '--sandbox', 'danger-full-access',
                          'Synthetic hook test. Follow the scripted calls.'])
            assert not ERRORS, ERRORS
            assert len(REQUESTS) == 6, (len(REQUESTS), result.stderr)
            initial = json.dumps(REQUESTS[0]['input'])
            assert 'git-fs is on' in initial, 'SessionStart guide missing'
            history = json.dumps(REQUESTS[-1]['input'])
            assert 'native apply_patch skips the guard' in history, 'Patch was not denied'
            assert 'git-fs read' in history, 'Shell guidance missing'
            assert 'overlay' in history, 'MCP read/write did not execute'
            assert 'routing is missing' not in history, 'MCP context injection failed'
            assert any(v.get('failed') == 0 and len(v.get('results', [])) == 2
                       for v in decoded_objects(REQUESTS[-1]['input'])), 'Successful batch result missing'
            assert (workspace / 'a.txt').read_text() == 'overlay\n', 'Stop did not materialize'
            refs = run(['git', 'for-each-ref', '--format=%(refname)', 'refs/gitfs/']).stdout.splitlines()
            assert len(refs) == 1 and refs[0] != 'refs/gitfs/main', refs
            print('PASS Codex host (' + ('code mode' if CODE_MODE else 'direct') + '): plugin startup, session guide, patch denial, MCP routing/batch/read/write, shell guidance, Stop materialization')
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
