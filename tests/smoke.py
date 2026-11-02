#!/usr/bin/env python3
"""Real Linux/WebKit/MPRIS smoke test. Run inside a graphical session.

Uses an isolated temporary profile and a local fixture server; no music-server
credentials or user settings are read. Requires xdotool and gdbus.
"""
import contextlib
import http.server
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
import wave

audio_buffer = io.BytesIO()
with wave.open(audio_buffer, 'wb') as audio_file:
    audio_file.setnchannels(1)
    audio_file.setsampwidth(2)
    audio_file.setframerate(8000)
    audio_file.writeframes(b'\0\0' * 8000 * 8)
AUDIO = audio_buffer.getvalue()

EVENTS = []
HTML = b'''<!doctype html><title>Cosine desktop smoke test</title>
<style>body{background:#0a0a0a;color:white;font:20px system-ui;padding:60px}</style>
<h1>Cosine desktop smoke test</h1><p id="result">Loading media bridge</p>
<button id="start" autofocus>Start silent audio test</button>
<script>
const ms = navigator.mediaSession;
ms.metadata = new MediaMetadata({title:'Smoke test track',artist:'Cosine',album:'Desktop tests'});
ms.playbackState = 'paused';
ms.setPositionState({duration:120,position:10});
const record = action => { fetch('/event?action='+action); document.querySelector('#result').textContent=action; };
const audio = new Audio('/silence.wav');
audio.addEventListener('playing',()=>{ms.playbackState='playing';record('audio-playing')});
audio.addEventListener('pause',()=>{ms.playbackState='paused'});
let progressed=false;
audio.addEventListener('timeupdate',()=>{if(audio.currentTime>1&&!progressed){progressed=true;record('audio-progressed')}});
document.querySelector('#start').onclick=()=>audio.play().catch(error=>record('audio-error'));
ms.setActionHandler('play', () => {audio.play();ms.playbackState='playing';record('play')});
ms.setActionHandler('pause', () => {audio.pause();ms.playbackState='paused';record('pause')});
ms.setActionHandler('nexttrack', () => record('next'));
ms.setActionHandler('seekto', details => {ms.setPositionState({duration:120,position:details.seekTime});record('seek-'+details.seekTime)});
window.__TAURI__.core.invoke('connection_settings').then(() => record('ACL-BYPASSED'), () => record('acl-denied'));
record('loaded');
</script>'''


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        if self.path == '/auth/config':
            body = b'{"localLoginEnabled":true,"oidcEnabled":false}'
            content_type = 'application/json'
        elif self.path == '/silence.wav':
            body, content_type = AUDIO, 'audio/wav'
        elif self.path.startswith('/event?'):
            EVENTS.append(urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)['action'][0])
            body, content_type = b'ok', 'text/plain'
        else:
            body, content_type = HTML, 'text/html'
        self.send_response(200)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def eventually(predicate, description, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.1)
    raise AssertionError(description)


def dbus(method, *args):
    return subprocess.run(['gdbus', 'call', '--session', '--dest', 'org.mpris.MediaPlayer2.cosine',
                           '--object-path', '/org/mpris/MediaPlayer2', '--method', method, *args],
                          capture_output=True, text=True, timeout=5)


def main():
    binary = str(Path(sys.argv[1] if len(sys.argv) > 1 else 'src-tauri/target/debug/cosine-desktop').resolve())
    with tempfile.TemporaryDirectory(prefix='cosine-desktop-smoke-') as temp:
        base = Path(temp)
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        config = base / 'config' / 'audio.cosine.desktop'
        config.mkdir(parents=True)
        address = f'http://127.0.0.1:{server.server_port}/'
        environment = dict(os.environ, XDG_CONFIG_HOME=str(base / 'config'), XDG_DATA_HOME=str(base / 'data'),
                           XDG_CACHE_HOME=str(base / 'cache'), GDK_BACKEND='x11')
        with (base / 'app.log').open('w+') as log:
            app = subprocess.Popen([binary], env=environment, stdout=log, stderr=log)
            try:
                def setup_window():
                    windows = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(app.pid)], capture_output=True, text=True)
                    return windows.stdout.splitlines() if windows.returncode == 0 else []
                eventually(lambda: bool(setup_window()), 'connection screen did not appear')
                window = setup_window()[0]
                # Mapping the GTK window precedes WebKit's autofocus and settings reply.
                time.sleep(1)
                subprocess.run(['xdotool', 'windowfocus', '--sync', window], check=True)
                subprocess.run(['xdotool', 'type', '--window', window, '--clearmodifiers', '--delay', '5', address], check=True)
                subprocess.run(['xdotool', 'key', '--window', window, 'Return'], check=True)
                eventually(lambda: 'loaded' in EVENTS, 'player did not load')
                # Activate native audio with a real user gesture in WebKit.
                eventually(lambda: bool(setup_window()), 'player window did not appear')
                subprocess.run(['xdotool', 'key', '--window', setup_window()[0], 'Return'], check=True)
                eventually(lambda: 'audio-progressed' in EVENTS, 'native WebKit audio did not advance')
                eventually(lambda: 'acl-denied' in EVENTS, 'remote page was not denied settings access')
                assert 'ACL-BYPASSED' not in EVENTS
                eventually(lambda: 'Smoke test track' in dbus('org.freedesktop.DBus.Properties.Get',
                            'org.mpris.MediaPlayer2.Player', 'Metadata').stdout, 'MPRIS metadata did not arrive')
                assert dbus('org.mpris.MediaPlayer2.Player.Play').returncode == 0
                eventually(lambda: 'play' in EVENTS, 'MPRIS play did not reach the web player')
                assert dbus('org.mpris.MediaPlayer2.Player.Pause').returncode == 0
                eventually(lambda: 'pause' in EVENTS, 'MPRIS pause did not reach the web player')
                assert dbus('org.mpris.MediaPlayer2.Player.Next').returncode == 0
                eventually(lambda: 'next' in EVENTS, 'MPRIS next did not reach the web player')
                # A second invocation must focus the existing process and exit.
                second = subprocess.run([binary], env=environment, capture_output=True, timeout=5)
                assert second.returncode == 0
                assert app.poll() is None
                print('PASS: real WebKit loads, silent audio plays and advances, connection works, remote ACL enforced,')
                print('      MPRIS metadata/play/pause/next work, second instance exits.')
            except Exception:
                windows = setup_window()
                if windows:
                    Path('artifacts').mkdir(exist_ok=True)
                    subprocess.run(['import', '-window', windows[0], 'artifacts/smoke-failure.png'], check=False)
                log.flush(); log.seek(0)
                print(log.read(), file=sys.stderr)
                print('Events:', EVENTS, file=sys.stderr)
                raise
            finally:
                if app.poll() is None:
                    app.terminate()
                    with contextlib.suppress(subprocess.TimeoutExpired):
                        app.wait(timeout=5)
                    if app.poll() is None:
                        app.kill(); app.wait()
                server.shutdown()


if __name__ == '__main__':
    main()
