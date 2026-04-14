// Tauri invoke bridge — uses real backend in app, stubs in browser dev mode
const isTauri = Boolean(window.__TAURI__);
const invoke = isTauri
  ? window.__TAURI__.core.invoke
  : async (cmd, args) => {
      console.debug('[stub invoke]', cmd, args);
      return stubResponses[cmd]?.(args) ?? null;
    };

// Platform detection: Tauri sets window.__TAURI_METADATA__ with platform info
const isAndroid = isTauri && (
  window.__TAURI_METADATA__?.currentPlatform === 'android' ||
  /android/i.test(navigator.userAgent)
);

// ── Stub responses for browser dev mode ──────────────────────────────────────
const stubResponses = {
  get_status: () => ({ status: 'disconnected', active_profile: null, bytes_up: 0, bytes_down: 0, elapsed_secs: 0 }),
  get_profiles: () => [],
  connect: () => null,
  disconnect: () => null,
  add_profile: ({ req }) => ({ id: crypto.randomUUID(), ...req }),
  deploy_server: () => ({ success: true, token: 'stub-token-abc123xyz', admin_token: 'stub-admin-token', message: 'stub' }),
  get_server_info: () => ({ status: 'running', version: '0.1.0', sessions: 3, uptime_secs: 3661 }),
  server_add_user: ({ req }) => ({ id: crypto.randomUUID(), label: req.label, token: 'user-tok-' + Math.random().toString(36).slice(2), is_admin: req.is_admin }),
  server_list_users: () => [],
  server_get_sessions: () => [],
};

// ── UI helpers ────────────────────────────────────────────────────────────────
const UI = {
  showModal(id) { document.getElementById(id)?.classList.remove('hidden'); },
  hideModal(id) { document.getElementById(id)?.classList.add('hidden'); },
  closeModalOutside(e, id) {
    if (e.target.classList.contains('modal-overlay')) this.hideModal(id);
  },
  log(text, isError = false) {
    const el = document.getElementById('deploy-log');
    el.classList.remove('hidden');
    const line = document.createElement('div');
    line.style.color = isError ? 'var(--red)' : '';
    line.textContent = `> ${text}`;
    el.appendChild(line);
    el.scrollTop = el.scrollHeight;
  },
  toast(msg, type = 'info') {
    // Simple toast notification
    const t = document.createElement('div');
    t.style.cssText = `
      position:fixed;bottom:20px;left:50%;transform:translateX(-50%);
      background:${type === 'error' ? 'var(--red-dim)' : 'var(--bg-card)'};
      border:1px solid ${type === 'error' ? 'var(--red)' : 'var(--border)'};
      border-radius:8px;padding:10px 18px;font-size:12px;
      z-index:999;animation:slide-up .2s ease;
    `;
    t.textContent = msg;
    document.body.appendChild(t);
    setTimeout(() => t.remove(), 2500);
  },
  formatBytes(n) {
    if (n < 1024) return n + ' B';
    if (n < 1024 ** 2) return (n / 1024).toFixed(1) + ' KB';
    if (n < 1024 ** 3) return (n / 1024 ** 2).toFixed(1) + ' MB';
    return (n / 1024 ** 3).toFixed(2) + ' GB';
  },
  formatTime(secs) {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    return h > 0
      ? `${h}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`
      : `${m}:${String(s).padStart(2,'0')}`;
  },
};

// ── App state ─────────────────────────────────────────────────────────────────
const App = {
  status: 'disconnected',
  profiles: [],
  selectedProfile: null,
  pollInterval: null,

  async init() {
    this.setupTabs();
    this.adaptForPlatform();
    await this.refreshProfiles();
    await this.pollStatus();
    this.pollInterval = setInterval(() => this.pollStatus(), 2000);
  },

  adaptForPlatform() {
    if (isAndroid) {
      // Hide Deploy tab — SSH deployment not available on mobile
      document.querySelector('.tab[data-tab="deploy"]')?.classList.add('hidden');
      // Hide autostart setting (not applicable on Android)
      document.querySelector('#s-autostart')?.closest('.setting-row')?.classList.add('hidden');
      // Show VPN permission banner if needed
      this.checkVpnPermission();
    }
  },

  async checkVpnPermission() {
    // On first launch, Android requires the user to grant VPN permission via system dialog.
    // We show a banner explaining this before the first connect attempt.
    const granted = localStorage.getItem('vpn_permission_granted');
    if (!granted) {
      const banner = document.createElement('div');
      banner.id = 'vpn-permission-banner';
      banner.style.cssText = `
        background: var(--bg-card);
        border: 1px solid var(--accent);
        border-radius: 10px;
        padding: 14px 16px;
        margin: 12px 16px;
        font-size: 13px;
        line-height: 1.5;
      `;
      banner.innerHTML = `
        <strong>VPN permission required</strong><br>
        <span style="color:var(--text-muted)">
          Veil needs your permission to create a VPN tunnel. A system dialog will appear when you first connect.
        </span>
      `;
      document.getElementById('tab-home').insertBefore(
        banner,
        document.getElementById('connect-btn-wrap')
      );
    }
  },

  setupTabs() {
    document.querySelectorAll('.tab').forEach(btn => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.tab').forEach(b => b.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
        btn.classList.add('active');
        document.getElementById('tab-' + btn.dataset.tab)?.classList.add('active');
        if (btn.dataset.tab === 'manage') App.refreshServerInfo();
      });
    });
  },

  async pollStatus() {
    try {
      const s = await invoke('get_status');
      this.applyStatus(s);
    } catch(e) {}
  },

  applyStatus(s) {
    this.status = typeof s.status === 'string' ? s.status : Object.keys(s.status)[0];
    const body = document.body;
    body.classList.remove('connected', 'connecting', 'error');
    if (this.status === 'connected')   body.classList.add('connected');
    if (this.status === 'connecting')  body.classList.add('connecting');

    const label = document.getElementById('status-label');
    const sub   = document.getElementById('status-sub');
    const btn   = document.getElementById('btn-connect');
    const stats = document.getElementById('stats-row');

    switch(this.status) {
      case 'connected':
        label.textContent = 'Connected';
        sub.textContent   = 'Your traffic is protected';
        btn.textContent   = 'Disconnect';
        stats.classList.remove('hidden');
        document.getElementById('stat-up').textContent   = UI.formatBytes(s.bytes_up);
        document.getElementById('stat-down').textContent = UI.formatBytes(s.bytes_down);
        document.getElementById('stat-time').textContent = UI.formatTime(s.elapsed_secs);
        // Animate shield check
        document.getElementById('shield-check').style.opacity = '1';
        break;
      case 'connecting':
        label.textContent = 'Connecting…';
        sub.textContent   = 'Establishing secure tunnel';
        btn.textContent   = 'Cancel';
        stats.classList.add('hidden');
        break;
      default:
        label.textContent = 'Disconnected';
        sub.textContent   = 'Your traffic is not protected';
        btn.textContent   = 'Connect';
        stats.classList.add('hidden');
        document.getElementById('shield-check').style.opacity = '0';
    }
  },

  async toggleConnect() {
    if (this.status === 'connected' || this.status === 'connecting') {
      if (isAndroid) {
        await invoke('stop_vpn');
      } else {
        await invoke('disconnect');
      }
    } else {
      if (!this.selectedProfile) {
        UI.toast('Select a server profile first', 'error');
        return;
      }
      if (isAndroid) {
        // Android: start_vpn triggers VpnService; system will show permission dialog on first use
        localStorage.setItem('vpn_permission_granted', '1');
        document.getElementById('vpn-permission-banner')?.remove();
        await invoke('start_vpn', { profileId: this.selectedProfile });
      } else {
        await invoke('connect', { req: { profile_id: this.selectedProfile, mode: 'vpn' } });
      }
    }
  },

  selectProfile(id) {
    this.selectedProfile = id || null;
    const badge = document.getElementById('mode-badge');
    if (id) {
      const p = this.profiles.find(p => p.id === id);
      badge.textContent = p?.mode?.toUpperCase() ?? 'VPN';
      badge.className = 'badge badge-blue';
      badge.classList.remove('hidden');
    } else {
      badge.classList.add('hidden');
    }
  },

  async refreshProfiles() {
    this.profiles = await invoke('get_profiles') ?? [];
    const sel = document.getElementById('profile-select');
    sel.innerHTML = '<option value="">— select server —</option>';
    this.profiles.forEach(p => {
      const opt = document.createElement('option');
      opt.value = p.id;
      opt.textContent = `${p.name}  (${p.host}:${p.port})`;
      sel.appendChild(opt);
    });
    this.renderProfilesList();
  },

  renderProfilesList() {
    const list = document.getElementById('profiles-list');
    if (!this.profiles.length) {
      list.innerHTML = '<div class="empty-state">No servers added yet</div>';
      return;
    }
    list.innerHTML = this.profiles.map(p => `
      <div class="list-item">
        <div class="list-item-icon">🖥</div>
        <div class="list-item-body">
          <div class="list-item-name">${p.name}</div>
          <div class="list-item-sub">${p.host}:${p.port} · ${p.mode.toUpperCase()}</div>
        </div>
        <div class="list-item-actions">
          <button class="btn-icon" style="font-size:12px" onclick="App.deleteProfile('${p.id}')" title="Delete">✕</button>
        </div>
      </div>
    `).join('');
  },

  async addProfile(e) {
    e.preventDefault();
    const req = {
      name:   document.getElementById('p-name').value,
      host:   document.getElementById('p-host').value,
      port:   parseInt(document.getElementById('p-port').value),
      token:  document.getElementById('p-token').value,
      domain: null,
      mode:   document.getElementById('p-mode').value,
    };
    const p = await invoke('add_profile', { req });
    if (p) {
      this.profiles.push(p);
      this.renderProfilesList();
      UI.hideModal('modal-add-profile');
      UI.toast('Server added');
      e.target.reset();
      this.refreshProfiles();
    }
  },

  async deleteProfile(id) {
    await invoke('delete_profile', { id });
    this.profiles = this.profiles.filter(p => p.id !== id);
    this.renderProfilesList();
    await this.refreshProfiles();
  },

  // ── Deploy ────────────────────────────────────────────────────────────────
  async deploy(e) {
    e.preventDefault();
    const btn = document.getElementById('btn-deploy');
    btn.disabled = true;
    btn.textContent = 'Deploying…';
    document.getElementById('deploy-log').innerHTML = '';
    document.getElementById('deploy-result').classList.add('hidden');

    const hostRaw = document.getElementById('d-host').value;
    const parts = hostRaw.includes('@') ? hostRaw.split('@') : ['root', hostRaw];

    const req = {
      host:      parts[1] || parts[0],
      ssh_port:  parseInt(document.getElementById('d-ssh-port').value),
      user:      parts[0],
      password:  document.getElementById('d-password').value || null,
      veil_port: parseInt(document.getElementById('d-veil-port').value),
      domain:    document.getElementById('d-domain').value || null,
    };

    UI.log('Connecting to ' + req.host + ':' + req.ssh_port + '…');
    UI.log('Checking requirements…');

    try {
      const result = await invoke('deploy_server', { req });
      UI.log('Container started!');
      this.showDeployResult(result, req);
    } catch(err) {
      UI.log('Error: ' + err, true);
    }

    btn.disabled = false;
    btn.textContent = 'Deploy Server';
  },

  showDeployResult(r, req) {
    const card = document.getElementById('deploy-result');
    card.classList.remove('hidden');
    card.innerHTML = `
      <h4>✓ Server deployed successfully</h4>
      <p style="font-size:12px;color:var(--text-muted);margin-bottom:10px">
        ${req.host}:${req.veil_port}
      </p>
      <div style="font-size:11px;color:var(--text-muted);margin-bottom:4px">User token (save this!)</div>
      <div class="token-row" onclick="App.copyToClipboard('${r.token}', this)">${r.token}</div>
    `;
    // Auto-add as profile
    this.profiles.push({
      id: crypto.randomUUID(),
      name: req.host,
      host: req.host,
      port: req.veil_port,
      token: r.token,
      mode: 'vpn',
    });
    this.refreshProfiles();
  },

  // ── Server management ─────────────────────────────────────────────────────
  async refreshServerInfo() {
    const card   = document.getElementById('server-info-card');
    const noSrv  = document.getElementById('no-server-msg');
    const actions = document.getElementById('manage-actions');

    try {
      const info = await invoke('get_server_info');
      noSrv.classList.add('hidden');
      card.classList.remove('hidden');
      actions.classList.remove('hidden');
      document.getElementById('srv-status').textContent = info.status;
      document.getElementById('srv-sessions').textContent = info.sessions;
      document.getElementById('srv-version').textContent = info.version;
      this.refreshUsers();
    } catch(e) {
      card.classList.add('hidden');
      actions.classList.add('hidden');
      noSrv.classList.remove('hidden');
    }
  },

  async addUser(e) {
    e.preventDefault();
    const req = {
      label:    document.getElementById('u-label').value,
      is_admin: document.getElementById('u-admin').checked,
    };
    const user = await invoke('server_add_user', { req });
    const result = document.getElementById('user-created');
    result.classList.remove('hidden');
    result.innerHTML = `
      <h4>User created: ${user.label}</h4>
      <div style="font-size:11px;color:var(--text-muted);margin-bottom:4px">Token:</div>
      <div class="token-row" onclick="App.copyToClipboard('${user.token}', this)">${user.token}</div>
    `;
    e.target.reset();
    this.refreshUsers();
  },

  async refreshUsers() {
    const users = await invoke('server_list_users') ?? [];
    const list = document.getElementById('users-list');
    if (!users.length) {
      list.innerHTML = '<div class="empty-state" style="padding:12px 0">No users yet</div>';
      return;
    }
    list.innerHTML = users.map(u => `
      <div class="list-item">
        <div class="list-item-icon">👤</div>
        <div class="list-item-body">
          <div class="list-item-name">${u.label}</div>
          <div class="list-item-sub">${u.is_admin ? 'Admin' : 'User'} · ${u.id.slice(0,8)}</div>
        </div>
      </div>
    `).join('');
  },

  async reloadServer() {
    UI.toast('Config reload scheduled');
  },

  // ── Settings ──────────────────────────────────────────────────────────────
  setSetting(key, value) {
    console.log('Setting:', key, '=', value);
    // Persist via Tauri store in full impl
  },

  // ── Utils ─────────────────────────────────────────────────────────────────
  async copyToClipboard(text, el) {
    try {
      await navigator.clipboard.writeText(text);
      const orig = el.style.color;
      el.style.color = 'var(--green)';
      setTimeout(() => el.style.color = orig, 800);
      UI.toast('Copied!');
    } catch(e) {}
  },
};

document.addEventListener('DOMContentLoaded', () => App.init());
