#!/usr/bin/env python3
"""Built-binary acceptance check. Only uses fixtures under src-tauri/target."""
import json
import os
from pathlib import Path
import select
import signal
import sqlite3
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'src-tauri/target/debug/query-studio-cli'


def receive(process):
    assert select.select([process.stdout], [], [], 5)[0], 'MCP response timed out'
    line = process.stdout.readline()
    assert line, f'MCP exited: {process.stderr.read()}'
    return json.loads(line)


def send(process, method, params=None, request_id=None):
    message = {'jsonrpc': '2.0', 'method': method}
    if params is not None:
        message['params'] = params
    if request_id is not None:
        message['id'] = request_id
    process.stdin.write(json.dumps(message) + '\n')
    process.stdin.flush()


with tempfile.TemporaryDirectory(prefix='automation-acceptance-', dir=ROOT / 'src-tauri/target') as directory:
    directory = Path(directory)
    data = directory / 'data.sqlite'
    with sqlite3.connect(data) as db:
        db.executescript("CREATE TABLE items(id INTEGER, name TEXT); INSERT INTO items VALUES(1,'one'),(2,'two'),(3,'three');")
    app = directory / 'query_studio.db'
    with sqlite3.connect(app) as db:
        db.executescript('CREATE TABLE connections(id TEXT, name TEXT, db_type TEXT, host TEXT, port TEXT, database_name TEXT, username TEXT, password TEXT, ssl_enabled INTEGER, auth_json TEXT, created_at TEXT, updated_at TEXT, options_json TEXT);')
        for connection_id in ['allowed', 'denied']:
            db.execute('INSERT INTO connections VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)', (connection_id, 'Fixture', 'sqlite', str(data), '', '', 'private-user', 'private-password', 0, 'private-auth', 'now', 'now', '{}'))
    original = app.read_bytes()
    args = [str(BINARY), '--app-db', str(app)]
    listed = subprocess.run(args + ['connections'], text=True, capture_output=True, check=True)
    assert len(json.loads(listed.stdout)['connections']) == 2
    assert 'private-' not in listed.stdout
    for sql in ['SELECT * FROM items ORDER BY id;', 'SELECT * FROM items ORDER BY id; -- final comment', 'SELECT * FROM items ORDER BY id; /* final comment */']:
        result = subprocess.run(args + ['--allow', 'allowed', 'query', '--connection', 'allowed', '--limit', '2'], input=sql, text=True, capture_output=True, check=True)
        assert json.loads(result.stdout)['truncated'] is True
        assert json.loads(result.stdout)['rows'][1]['name'] == 'two'
    rejected = subprocess.run(args + ['--allow', 'allowed', 'query', '--connection', 'allowed'], input='DELETE FROM items', text=True, capture_output=True)
    assert rejected.returncode == 1 and not rejected.stdout
    process = subprocess.Popen(args + ['--allow', 'allowed', 'mcp'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        send(process, 'initialize', {'protocolVersion':'2025-11-25', 'capabilities':{}, 'clientInfo':{'name':'acceptance','version':'1'}}, 1)
        assert receive(process)['result']['protocolVersion'] == '2025-11-25'
        send(process, 'notifications/initialized')
        send(process, 'tools/list', request_id=2)
        assert len(receive(process)['result']['tools']) == 4
        send(process, 'tools/call', {'name':'list_connections','arguments':{}}, 3)
        listed = receive(process)['result']
        assert listed['isError'] is False
        assert len(json.loads(listed['content'][0]['text'])['connections']) == 1
        assert 'private-' not in json.dumps(listed)
        send(process, 'tools/call', {'name':'query','arguments':{'connection_id':'denied','query':'SELECT 1'}}, 4)
        assert receive(process)['result']['isError'] is True
        send(process, 'tools/call', {'name':'query','arguments':{'connection_id':'allowed','query':'SELECT count(*) AS n FROM items'}}, 5)
        assert json.loads(receive(process)['result']['content'][0]['text'])['rows'][0]['n'] == 3
        process.stdin.write('not json\n'); process.stdin.flush()
        assert receive(process)['error']['code'] == -32700
        process.stdin.write('x' * (1024 * 1024 + 1) + '\n'); process.stdin.flush()
        assert receive(process)['error']['code'] == -32700
        assert process.wait(timeout=5) == 1
    finally:
        if process.poll() is None:
            process.terminate(); process.wait(timeout=5)
    assert app.read_bytes() == original, 'App database was modified'
    with sqlite3.connect(data) as db:
        assert db.execute('SELECT count(*) FROM items').fetchone()[0] == 3
    if os.name == 'posix':
        # Fake only the OpenSSH executable. No SSH peer or network database is used.
        ssh = directory / 'ssh'
        pid_file = directory / 'ssh.pid'
        ssh.write_text('#!/bin/sh\nfor arg in "$@"; do [ "$arg" = "-O" ] && exit 0; done\necho $$ > "$QS_FIXTURE_PID"\nexec /bin/sleep 60\n')
        ssh.chmod(0o700)
        options = {'ssh':{'host':'fixture.invalid','port':22,'username':'fixture','identity_file':None}}
        with sqlite3.connect(app) as db:
            db.execute("UPDATE connections SET db_type='postgres', host='127.0.0.1', port='1', options_json=? WHERE id='allowed'", (json.dumps(options),))
        env = dict(os.environ, PATH=str(directory) + os.pathsep + os.environ['PATH'], QS_FIXTURE_PID=str(pid_file))
        process = subprocess.Popen(args + ['--allow', 'allowed', 'mcp'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        try:
            send(process, 'initialize', {'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'fixture','version':'1'}}, 1); receive(process)
            send(process, 'notifications/initialized')
            send(process, 'tools/call', {'name':'query','arguments':{'connection_id':'allowed','query':'SELECT 1'}}, 2)
            assert receive(process)['result']['isError'] is True
            ssh_pid = int(pid_file.read_text())
            os.kill(ssh_pid, 0)  # Owned forwarding child is still cached before shutdown.
            process.send_signal(signal.SIGTERM)
            assert process.wait(timeout=5) == 1
            try:
                os.kill(ssh_pid, 0)
                raise AssertionError('Forwarding child survived SIGTERM')
            except ProcessLookupError:
                pass
        finally:
            if process.poll() is None:
                process.terminate(); process.wait(timeout=5)
            if pid_file.exists():
                try:
                    os.kill(int(pid_file.read_text()), signal.SIGKILL)
                except ProcessLookupError:
                    pass
print('CLI/MCP native SQLite, transport limits, redaction, and graceful SSH cleanup passed')
