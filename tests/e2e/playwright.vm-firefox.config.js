// VM-local override: run the N6.2 suite under Firefox because the VM's egress
// proxy refuses Chromium's CONNECT tunnels (ERR_TUNNEL_CONNECTION_FAILED) while
// Firefox tunnels fine. Chromium remains the canonical CI project; this file is
// NOT part of the suite and is deleted after the run.
import base from './playwright.config.js';
const u = new URL(process.env.HTTPS_PROXY);
export default {
  ...base,
  projects: [{
    name: 'firefox-vm',
    use: {
      browserName: 'firefox',
      viewport: { width: 1440, height: 900 },
      proxy: {
        server: `${u.protocol}//${u.host}`,
        username: decodeURIComponent(u.username),
        password: decodeURIComponent(u.password),
      },
    },
  }],
};
