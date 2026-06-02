const nativeFetch = window.fetch.bind(window);

let authState = {
    enabled: false,
    methods: { basic: false, form: false },
    authenticated: true,
    method: null,
    username: null,
};

function authShellElements() {
    return {
        shell: document.getElementById('auth-shell'),
        form: document.getElementById('auth-shell-form'),
        username: document.getElementById('auth-username'),
        password: document.getElementById('auth-password'),
        submit: document.getElementById('auth-submit'),
        error: document.getElementById('auth-shell-error'),
        note: document.getElementById('auth-shell-note'),
        recovery: document.getElementById('auth-shell-recovery'),
        subtitle: document.getElementById('auth-shell-subtitle'),
        badges: document.getElementById('auth-shell-badges'),
    };
}

function setAuthShellVisible(visible) {
    const { shell } = authShellElements();
    if (!shell) return;
    shell.classList.toggle('hidden', !visible);
    shell.setAttribute('aria-hidden', visible ? 'false' : 'true');
    document.body.classList.toggle('auth-required', visible);
}

function renderMethodBadges(methods) {
    const { badges } = authShellElements();
    if (!badges) return;
    badges.replaceChildren();
    if (methods.form) badges.appendChild(buildBadge('Form Login Enabled'));
    if (methods.basic) badges.appendChild(buildBadge('Basic Auth Available'));
}

const _PERSON_SVG = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="8" r="4"/><path d="M4 20c0-4 3.6-7 8-7s8 3 8 7"/></svg>`;
const _LOCK_SVG = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>`;

function _navUserLabelSpan(text) {
    const s = document.createElement('span');
    s.className = 'nav-user-btn-label';
    s.textContent = text;
    return s;
}

function updateUserLabel() {
    const btn = document.getElementById('nav-user-btn');
    if (!btn) return;

    if (authState.authenticated && authState.username) {
        // eslint-disable-next-line no-unsanitized/property -- static SVG, no user data
        btn.innerHTML = _PERSON_SVG;
        btn.appendChild(_navUserLabelSpan(authState.username));
        btn.classList.remove('nav-user-btn-icon');
        return;
    }
    if (authState.methods.form) {
        // eslint-disable-next-line no-unsanitized/property -- static SVG, no user data
        btn.innerHTML = _LOCK_SVG;
        btn.appendChild(_navUserLabelSpan('Sign In'));
        btn.classList.remove('nav-user-btn-icon');
        return;
    }
    // Anonymous / no auth required — icon only, no label
    // eslint-disable-next-line no-unsanitized/property -- static SVG, no user data
    btn.innerHTML = _PERSON_SVG;
    btn.classList.add('nav-user-btn-icon');
}

function buildBadge(text) {
    const badge = document.createElement('span');
    badge.className = 'auth-shell-badge';
    badge.textContent = text;
    return badge;
}

function renderAuthShell() {
    const { note, recovery, subtitle, error, username } = authShellElements();
    if (error) error.textContent = '';
    renderMethodBadges(authState.methods || {});
    if (subtitle) {
        subtitle.textContent = authState.methods.basic && authState.methods.form
            ? 'Use the in-app sign-in form, or authenticate with HTTP Basic Auth if you prefer.'
            : 'Enter your credentials to unlock protected routes.';
    }
    if (note) {
        if (authState.methods.basic && authState.methods.form) {
            note.textContent = 'This server accepts either a form session or HTTP Basic credentials.';
        } else if (authState.methods.basic) {
            note.textContent = 'This server is configured for browser-level Basic Auth.';
        } else {
            note.textContent = 'Protected API routes stay locked until this form succeeds.';
        }
    }
    if (recovery) {
        recovery.textContent = authState.managedByCli
            ? 'Recovery is managed by startup flags for this instance.'
            : `Forgot the password? Run "${authState.recoveryCommand || 'llama-monitor --clear-auth-config'}" on this machine, restart, and sign in again.`;
    }
    if (username) username.focus();
}

async function fetchAuthStatus() {
    try {
        const res = await nativeFetch('/api/auth/status', { cache: 'no-store', credentials: 'same-origin' });
        if (!res.ok) return authState;
        authState = await res.json();
        updateUserLabel();
        return authState;
    } catch {
        return authState;
    }
}

async function submitFormLogin(event) {
    event?.preventDefault();
    const { username, password, submit, error } = authShellElements();
    if (!username || !password || !submit || !error) return;
    error.textContent = '';
    submit.disabled = true;
    submit.textContent = 'Signing In…';
    try {
        const res = await nativeFetch('/api/auth/login', {
            method: 'POST',
            credentials: 'same-origin',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                username: username.value.trim(),
                password: password.value,
            }),
        });
        if (!res.ok) {
            const data = await res.json().catch(() => ({}));
            error.textContent = data.error === 'invalid_credentials'
                ? 'Those credentials did not match the configured account.'
                : 'Sign-in failed.';
            return;
        }
        window.location.reload();
    } finally {
        submit.disabled = false;
        submit.textContent = 'Sign In';
    }
}

function installFetchInterceptor() {
    window.fetch = async (...args) => {
        const response = await nativeFetch(...args);
        const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
        if (
            response.status === 401
            && !url.includes('/api/auth/login')
            && !url.includes('/api/auth/status')
            && authState.methods?.form
        ) {
            await fetchAuthStatus();
            if (!authState.authenticated) {
                renderAuthShell();
                setAuthShellVisible(true);
            }
        }
        return response;
    };
}

export async function initAuthGate() {
    installFetchInterceptor();
    const form = document.getElementById('auth-shell-form');
    form?.addEventListener('submit', submitFormLogin);

    const status = await fetchAuthStatus();
    if (status.methods?.form && !status.authenticated) {
        renderAuthShell();
        setAuthShellVisible(true);
        return { ready: false, state: status };
    }

    setAuthShellVisible(false);
    return { ready: true, state: status };
}

export async function logoutCurrentUser() {
    await nativeFetch('/api/auth/logout', {
        method: 'POST',
        credentials: 'same-origin',
    }).catch(() => null);
    window.__API_TOKEN = null;
    window.location.reload();
}
