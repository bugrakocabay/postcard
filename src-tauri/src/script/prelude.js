// Builds `pm` and `console` for user scripts. Evaluated once per run, after the
// host has defined `__state` (JSON string), `__log(level, msg)` and
// `__shell(cmd)` (returns a JSON string `{ok, out}` / `{ok:false, err}`).
// Defines `__finish()`, which the host calls to read the final state back.
(function () {
  "use strict";
  const g = globalThis;
  const hostState = JSON.parse(g.__state);
  const hostLog = g.__log;
  const hostShell = g.__shell;
  delete g.__state;
  delete g.__log;
  delete g.__shell;

  const toMap = (obj) => new Map(Object.entries(obj || {}));
  const toStr = (v) =>
    typeof v === "string"
      ? v
      : v !== null && typeof v === "object"
        ? JSON.stringify(v)
        : String(v);
  const lower = (s) => String(s).toLowerCase();

  // ---- variable scopes ----------------------------------------------------
  const env = toMap(hostState.env);
  const globals = toMap(hostState.globals);
  const runtime = toMap(hostState.runtime);

  const scope = (map, withUnset) => {
    const s = {
      get: (k) => map.get(String(k)),
      set: (k, v) => {
        map.set(String(k), toStr(v));
      },
      has: (k) => map.has(String(k)),
      toObject: () => Object.fromEntries(map),
    };
    if (withUnset) s.unset = (k) => void map.delete(String(k));
    return s;
  };

  const variables = {
    get(k) {
      k = String(k);
      for (const m of [runtime, env, globals]) if (m.has(k)) return m.get(k);
      return undefined;
    },
    set: (k, v) => void runtime.set(String(k), toStr(v)),
    has: (k) => [runtime, env, globals].some((m) => m.has(String(k))),
    unset: (k) => void runtime.delete(String(k)),
    toObject: () => Object.fromEntries([...globals, ...env, ...runtime]),
  };

  // ---- header lists -------------------------------------------------------
  const def = (obj, name, fn) =>
    Object.defineProperty(obj, name, {
      value: fn,
      enumerable: false,
      writable: true,
      configurable: true,
    });

  // Accepts ({key, value}) or (key, value).
  const entry = (a, b) => {
    const key = a !== null && typeof a === "object" ? a.key : a;
    const value = a !== null && typeof a === "object" ? a.value : b;
    return { key: String(key), value: String(value) };
  };

  const headerList = (pairs, mutable) => {
    const list = (pairs || []).map(([key, value]) => ({ key, value }));
    const find = (name) => list.findIndex((h) => lower(h.key) === lower(name));
    def(list, "get", (name) => {
      const i = find(name);
      return i < 0 ? undefined : list[i].value;
    });
    def(list, "has", (name) => find(name) >= 0);
    def(list, "toObject", () => {
      const o = {};
      for (const h of list) o[h.key] = h.value;
      return o;
    });
    if (mutable) {
      def(list, "add", (a, b) => void list.push(entry(a, b)));
      def(list, "upsert", (a, b) => {
        const h = entry(a, b);
        const i = find(h.key);
        if (i < 0) {
          list.push(h);
          return;
        }
        list[i] = h;
        for (let j = list.length - 1; j > i; j--) {
          if (lower(list[j].key) === lower(h.key)) list.splice(j, 1);
        }
      });
      def(list, "remove", (name) => {
        for (let j = list.length - 1; j >= 0; j--) {
          if (lower(list[j].key) === lower(name)) list.splice(j, 1);
        }
      });
    }
    return list;
  };

  // ---- request / response -------------------------------------------------
  const rq = hostState.request || {};
  const request = {
    method: rq.method,
    url: rq.url,
    headers: headerList(rq.headers, true),
    body: rq.body === null ? undefined : rq.body,
  };

  let response;
  const rs = hostState.response;
  if (rs) {
    const headers = headerList(rs.headers, false);
    // Repeated headers (e.g. Set-Cookie) are joined, like fetch does.
    def(headers, "get", (name) => {
      const all = headers.filter((h) => lower(h.key) === lower(name));
      return all.length ? all.map((h) => h.value).join(", ") : undefined;
    });
    response = {
      code: rs.code,
      status: rs.status,
      headers,
      responseTime: rs.timeMs,
      text: () => rs.body,
      json: () => JSON.parse(rs.body),
    };
  }

  // ---- shell --------------------------------------------------------------
  const shell = (cmd) => {
    const r = JSON.parse(hostShell(String(cmd)));
    if (!r.ok) throw new Error(r.err);
    return r.out;
  };

  // ---- console ------------------------------------------------------------
  const fmt = (v) => {
    if (typeof v === "string") return v;
    if (v instanceof Error) return v.name + ": " + v.message;
    try {
      const s = JSON.stringify(v);
      return s === undefined ? String(v) : s;
    } catch (_) {
      return String(v);
    }
  };
  const logger = (level) => (...args) => hostLog(level, args.map(fmt).join(" "));
  g.console = {
    log: logger("log"),
    debug: logger("log"),
    info: logger("info"),
    warn: logger("warn"),
    error: logger("error"),
  };

  g.pm = {
    environment: scope(env, true),
    globals: scope(globals, true),
    variables,
    request,
    response,
    shell,
  };

  // ---- hand the state back to the host ------------------------------------
  g.__finish = () => {
    const body = request.body;
    const pairs = [];
    for (const h of Array.from(request.headers)) {
      if (h && h.key !== undefined) pairs.push([String(h.key), String(h.value)]);
    }
    return JSON.stringify({
      env: Object.fromEntries(env),
      globals: Object.fromEntries(globals),
      runtime: Object.fromEntries(runtime),
      request: {
        method: String(request.method),
        url: String(request.url),
        headers: pairs,
        body: body === undefined || body === null ? null : toStr(body),
      },
    });
  };
})();
