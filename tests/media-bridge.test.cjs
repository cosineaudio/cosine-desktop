const { test } = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const source = fs.readFileSync('src-tauri/src/media-bridge.js', 'utf8')
  .replace('__COSINE_ORIGIN__', '"https://music.example.com"');

function fixture(origin = 'https://music.example.com') {
  const updates = [], nativeHandlers = new Map(), timers = new Map();
  let id = 0;
  class Session {
    get metadata() { return this._metadata; }
    set metadata(value) { this._metadata = value; }
    get playbackState() { return this._state || 'none'; }
    set playbackState(value) { this._state = value; }
    setActionHandler(action, handler) { nativeHandlers.set(action, handler); }
    setPositionState(state) { this.originalPosition = state; }
  }
  const session = new Session();
  const window = { __TAURI__: { core: { invoke: async (command, value) => { updates.push({ command, ...value }); } } } };
  window.top = window;
  vm.runInNewContext(source, { window, navigator: { mediaSession: session }, location: { origin },
    setTimeout: callback => { timers.set(++id, callback); return id; },
    clearTimeout: id => timers.delete(id) });
  return { window, session, updates, nativeHandlers, timers };
}

test('preserves browser MediaSession and forwards metadata and state', () => {
  const f = fixture();
  f.session.metadata = { title: 'Song', artist: 'Artist', album: 'Album' };
  f.session.playbackState = 'playing';
  assert.equal(f.session.metadata.title, 'Song');
  assert.equal(f.session.playbackState, 'playing');
  assert.equal(f.updates.at(-1).snapshot.title, 'Song');
  assert.equal(f.updates.at(-1).snapshot.state, 'playing');
  assert.equal(f.updates.at(-1).command, 'publish_media');
});

test('coalesces position updates without a persistent polling loop', () => {
  const f = fixture();
  for (let position = 0; position < 100; position++) f.session.setPositionState({ duration: 200, position });
  assert.equal(f.session.originalPosition.position, 99);
  assert.equal(f.timers.size, 1);
  assert.equal(f.updates.length, 0);
  const callback = f.timers.values().next().value;
  f.timers.clear(); callback();
  assert.equal(f.updates[0].snapshot.position, 99);
  assert.equal(f.timers.size, 0);
});

test('desktop controls call the existing player handlers with seek details', () => {
  const f = fixture();
  let called;
  const handler = details => { called = details; };
  f.session.setActionHandler('seekto', handler);
  assert.equal(f.nativeHandlers.get('seekto'), handler);
  f.window.__cosineDesktopMedia('seekto', 42);
  assert.equal(called.seekTime, 42);
  f.session.setActionHandler('pause', handler);
  f.session.playbackState = 'playing';
  f.window.__cosineDesktopMedia('toggle', 0);
  assert.equal(called.action, 'pause');
  f.session.setActionHandler('pause', null);
  called = null;
  f.window.__cosineDesktopMedia('pause', 0);
  assert.equal(called, null);
});

test('does not install on an identity provider or other external origin', () => {
  const f = fixture('https://identity.example.com');
  f.session.metadata = { title: 'not a player' };
  assert.equal(f.updates.length, 0);
  assert.equal(f.window.__cosineDesktopMedia, undefined);
});

test('a new track clears the previous track position', () => {
  const f = fixture();
  f.session.setPositionState({ duration: 200, position: 99 });
  f.session.metadata = { title: 'Next song' };
  assert.equal(f.updates.at(-1).snapshot.position, 0);
  assert.equal(f.timers.size, 0);
});
