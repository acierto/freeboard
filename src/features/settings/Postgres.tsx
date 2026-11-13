// Settings → Postgres databases (supplemental; the prototype has no such
// group): what Freeboard looks for, the local servers it looks in, how to
// give it a password without Freeboard storing one, and a Check that tries
// every server now. The deep scan reads the same servers
// (detectors/postgres.rs).
import { Fragment, useState } from "react";
import { api } from "../../lib/api";
import { gb } from "../../lib/format";
import { track } from "../../lib/useBusy";
import type { Platform, PostgresCheck, PostgresServer, Settings, SettingsPatch } from "../../lib/types";
import { t } from "../../i18n";

const HOSTS: PostgresServer["host"][] = ["localhost", "127.0.0.1", "::1"];

/** A translated sentence with `{name}` holes filled by monospaced text. */
function withCode(text: string, code: Record<string, string>) {
  const parts = text.split(/\{(\w+)\}/);
  return parts.map((p, i) =>
    i % 2 === 1 ? (
      <span key={i} className="mono">
        {code[p] ?? p}
      </span>
    ) : (
      <Fragment key={i}>{p}</Fragment>
    ),
  );
}

/** "localhost:5433 as dev", or the default server. */
function serverLabel(s: { host: string; port: string | number; user: string }): string {
  if (!s.host && !s.user) return s.port ? t("s.pg.default.port", { p: String(s.port) }) : t("s.pg.default");
  const where = `${s.host || "localhost"}${s.port ? ":" + s.port : ""}`;
  return s.user ? t("s.pg.as", { s: where, u: s.user }) : where;
}

function checkText(c: PostgresCheck): string {
  switch (c.state) {
    case "ok":
      return c.databases === 0 ? t("s.pg.ok0") : c.databases === 1 ? t("s.pg.ok1", { x: gb(c.bytes) }) : t("s.pg.okN", { n: c.databases, x: gb(c.bytes) });
    case "absent":
      return t("s.pg.absent");
    case "refused":
      return t("s.pg.refused", { m: c.message ?? "" });
    default:
      return t("s.pg.nopsql");
  }
}

export function PostgresSection({ settings, platform, onChange }: { settings: Settings; platform: Platform; onChange: (p: SettingsPatch) => void }) {
  const servers = settings.postgresServers ?? [];
  const [host, setHost] = useState<PostgresServer["host"]>("localhost");
  const [port, setPort] = useState("5432");
  const [user, setUser] = useState("");
  const [checks, setChecks] = useState<PostgresCheck[] | null>(null);
  const portNum = Number(port);
  const userOk = user === "" || /^[A-Za-z0-9_.][A-Za-z0-9_.-]{0,62}$/.test(user);
  const dup = servers.some((s) => s.host === host && s.port === portNum && s.user === user);
  const valid = Number.isInteger(portNum) && portNum >= 1 && portNum <= 65535 && userOk && !dup && servers.length < 10;
  const set = (next: PostgresServer[]) => {
    setChecks(null);
    onChange({ postgresServers: next });
  };
  const win = platform === "windows";
  const file = win ? "%APPDATA%\\postgresql\\pgpass.conf" : "~/.pgpass";

  return (
    <section className="st-sec pg-sec" aria-labelledby="st-pg" data-supplemental="postgres">
      <h3 id="st-pg">{t("s.pg.h")}</h3>
      <p>{t("s.pg.p")}</p>
      <p>{withCode(t("s.pg.names"), { ex: "myapp_wt3", a: "wt3", b: "myapp-wt3" })}</p>

      <div className="st-sub">{t("s.pg.servers")}</div>
      <div className="st-list" id="pg-servers">
        <div className="frow setrow">
          <span>
            <b>{t("s.pg.default")}</b>
            <small>{t(win ? "s.pg.default.win" : "s.pg.default.s")}</small>
          </span>
        </div>
        {servers.map((s, i) => (
          <div className="frow setrow" key={`${s.host}:${s.port}:${s.user}`}>
            <span>
              <span className="path mono">{serverLabel(s)}</span>
              <small>{t("s.pg.added")}</small>
            </span>
            <button className="icon-x" id={`pg-rm-${i}`} aria-label={t("a.pg.rm", { s: serverLabel(s) })} title={t("a.remove")} onClick={() => set(servers.filter((_, j) => j !== i))}>
              ×
            </button>
          </div>
        ))}
      </div>

      <div className="pg-add" role="group" aria-label={t("s.pg.add")}>
        <label>
          <small>{t("s.pg.host")}</small>
          <select id="pg-host" value={host} onChange={(e) => setHost(e.target.value as PostgresServer["host"])}>
            {HOSTS.map((h) => (
              <option key={h} value={h}>
                {h}
              </option>
            ))}
          </select>
        </label>
        <label>
          <small>{t("s.pg.port")}</small>
          <input id="pg-port" inputMode="numeric" value={port} size={6} onChange={(e) => setPort(e.target.value.replace(/\D/g, "").slice(0, 5))} />
        </label>
        <label>
          <small>{t("s.pg.user")}</small>
          <input id="pg-user" value={user} placeholder={t("s.pg.user.ph")} size={16} autoCapitalize="off" spellCheck={false} onChange={(e) => setUser(e.target.value.trim())} />
        </label>
        <button className="btn" id="pg-add" disabled={!valid} onClick={() => {
            set([...servers, { host, port: portNum, user }]);
            setPort("5432");
            setUser("");
          }}>
          {t("s.pg.add")}
        </button>
      </div>
      {dup && <p className="note">{t("s.pg.dup")}</p>}
      {!userOk && <p className="note">{t("s.pg.baduser")}</p>}

      <div className="st-sub">{t("s.pg.pw.h")}</div>
      <p>{withCode(t(win ? "s.pg.pw.win" : "s.pg.pw.p"), { f: file, c: "chmod 600 ~/.pgpass" })}</p>
      <pre className="mono pg-example" aria-label={t("s.pg.example")}>
        {"# host:port:database:user:password\nlocalhost:5432:*:dev:your-password\n127.0.0.1:5433:*:app:another-password"}
      </pre>
      <p className="muted small">{t("s.pg.example.s")}</p>

      <div>
        <button className="btn" id="pg-check" onClick={() => void track(api.checkPostgres()).then(setChecks, () => undefined)}>
          {t("s.pg.check")}
        </button>
      </div>
      {checks && (
        <ul className="pg-checks" id="pg-results" aria-live="polite">
          {checks.map((c) => (
            <li key={`${c.host}:${c.port}:${c.user}`} className={c.state === "ok" ? "ok" : "no"}>
              <b>{serverLabel(c)}</b>: {checkText(c)}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
