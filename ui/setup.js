/* The player itself is the unchanged Vue app served by Cosine. This script only
   runs in the bundled, privileged connection window. */
const { invoke } = window.__TAURI__.core;
const form = document.querySelector('#connection');
const server = document.querySelector('#server');
const button = document.querySelector('#connect');
const status = document.querySelector('#status');

async function connect() {
  if (button.disabled) return;
  button.disabled = true;
  server.disabled = true;
  form.setAttribute('aria-busy', 'true');
  button.textContent = 'Connecting…';
  status.className = '';
  status.textContent = 'Connecting to your server…';
  try {
    await invoke('connect_server', { address: server.value });
    status.textContent = '';
  } catch (error) {
    status.className = 'error';
    status.textContent = String(error);
  } finally {
    button.disabled = false;
    server.disabled = false;
    form.setAttribute('aria-busy', 'false');
    button.textContent = 'Connect';
  }
}
form.addEventListener('submit', event => { event.preventDefault(); connect(); });
invoke('connection_settings').then(settings => {
  // Do not overwrite an address already being typed while settings load.
  if (!server.value) server.value = settings.server || '';
  if (settings.error) {
    status.className = 'error';
    status.textContent = settings.error;
  }
}).catch(error => {
  status.textContent = String(error);
  status.className = 'error';
});
