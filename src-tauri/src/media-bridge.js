// Installed before Cosine runs. Keep the original UI, audio engine, MediaSession
// and action handlers intact; mirror their state to Linux's MPRIS interface.
(() => {
  if (window.top !== window || location.origin !== __COSINE_ORIGIN__) return;
  const session = navigator.mediaSession;
  const invoke = window.__TAURI__?.core?.invoke;
  if (!session || !invoke) return;
  const handlers = new Map();
  let position = { duration: 0, position: 0 };
  let pending = 0;
  const publish = () => {
    pending = 0;
    const metadata = session.metadata;
    invoke('publish_media', { snapshot: {
      title: metadata?.title || '', artist: metadata?.artist || '',
      album: metadata?.album || '', state: session.playbackState,
      duration: Number.isFinite(position.duration) ? position.duration : 0,
      position: Number.isFinite(position.position) ? position.position : 0,
    } }).catch(() => {});
  };
  // Position updates arrive on audio timeupdate; coalesce them to at most 1/s.
  // No polling and no timer is left running while playback is idle.
  const schedule = () => { if (!pending) pending = setTimeout(publish, 1000); };
  const immediate = () => { clearTimeout(pending); pending = 0; publish(); };
  const prototype = Object.getPrototypeOf(session);
  for (const property of ['metadata', 'playbackState']) {
    const descriptor = Object.getOwnPropertyDescriptor(prototype, property);
    if (descriptor?.get && descriptor?.set) Object.defineProperty(session, property, {
      configurable: true,
      get: () => descriptor.get.call(session),
      set: value => {
        descriptor.set.call(session, value);
        if (property === 'metadata') position = { duration: 0, position: 0 };
        immediate();
      },
    });
  }
  const setActionHandler = session.setActionHandler.bind(session);
  session.setActionHandler = (action, handler) => {
    if (handler) handlers.set(action, handler); else handlers.delete(action);
    setActionHandler(action, handler);
  };
  if (session.setPositionState) {
    const setPositionState = session.setPositionState.bind(session);
    session.setPositionState = state => {
      setPositionState(state);
      position = state || { duration: 0, position: 0 };
      schedule();
    };
  }
  Object.defineProperty(window, '__cosineDesktopMedia', { value: (action, seconds) => {
    if (action === 'toggle') action = session.playbackState === 'playing' ? 'pause' : 'play';
    if (action === 'stop' && !handlers.has('stop')) action = 'pause';
    const handler = handlers.get(action);
    if (handler) handler({ action, seekTime: seconds, seekOffset: seconds || 10 });
  } });
})();
