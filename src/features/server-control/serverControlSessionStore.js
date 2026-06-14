const STATUS_CACHE_TTL_MS = 30_000;

const EMPTY_STATUS = {
  checked: false,
  running: false,
  ready: false,
  statusMessage: '',
  error: '',
  cachedAt: 0,
  bootTracking: false,
  bootStartedAt: 0
};

/** @type {Map<string, typeof EMPTY_STATUS>} */
const sessions = new Map();

export function serverControlScopeKey(sshHost, serverRootPath) {
  return `${(sshHost || '').trim().toLowerCase()}::${(serverRootPath || '').trim()}`;
}

export function readServerControlSession(scopeKey) {
  const stored = sessions.get(scopeKey);
  if (!stored) {
    return { ...EMPTY_STATUS };
  }
  return { ...stored };
}

export function writeServerControlSession(scopeKey, patch) {
  const next = { ...readServerControlSession(scopeKey), ...patch };
  sessions.set(scopeKey, next);
  return next;
}

export function resetServerControlSession(scopeKey) {
  sessions.set(scopeKey, { ...EMPTY_STATUS });
  return readServerControlSession(scopeKey);
}

export function serverControlStatusCacheTtlMs() {
  return STATUS_CACHE_TTL_MS;
}

export function isServerControlStatusFresh(session, now = Date.now()) {
  if (!session?.checked || !session.cachedAt) {
    return false;
  }
  return now - session.cachedAt <= STATUS_CACHE_TTL_MS;
}

export function isServerBootInProgress(session) {
  return Boolean(session?.bootTracking && !session.ready);
}

export function bootElapsedSeconds(startedAt, now = Date.now()) {
  if (!startedAt) {
    return 0;
  }
  return Math.max(1, Math.ceil((now - startedAt) / 1000));
}
