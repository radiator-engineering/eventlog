// Pure helpers: no Herdr imports, so `node --test` can load them.

/** Fields every event carries that the panel never shows as content. */
export const PLUMBING = new Set(["v", "prev", "seq", "ts", "type"]);

/** Types shown as their own filter chip; everything else falls under "other". */
export const MAIN_TYPES = ["result", "decision", "note", "spawn", "progress", "escalate", "message"];

/** Types hidden unless the person asks for them: context rebuilds are noise for a reader. */
export const QUIET_TYPES = new Set(["rebuild", "ack", "intent"]);

/** One line that says what an event is about. */
export function headline(e) {
  if (e.type === "decision") return `${e.key ?? "?"} = ${e.value ?? ""}`;
  if (e.type === "rebuild") return `context rebuilt (${e.trigger ?? "?"}, ${e.reason ?? ""})`;
  if (e.type === "seam") return `${e.agents ?? ""}: ${e.subject ?? ""}`;
  if (e.type === "claim") return `claims ${e.paths ?? ""}`;
  if (e.type === "retire") return `retired ${e.disposition ?? ""}`;
  if (e.type === "ack") return `${e.outcome ?? ""} #${e.seq_done ?? ""}${e.detail ? ` ${e.detail}` : ""}`;
  const text = e.summary ?? e.msg ?? e.subject ?? e.value ?? e.reason ?? "";
  if (text) return String(text);
  if (e.ref) return String(e.ref);
  return "";
}

/** Who an event is from or about. */
export function actor(e) {
  return e.agent || e.by || (e.type === "decision" || e.type === "note" ? "controller" : "");
}

/** "now", "4m", "3h", "2d". */
export function ago(ts, nowMs) {
  const t = Date.parse(ts);
  if (Number.isNaN(t)) return "";
  const s = Math.max(0, Math.round((nowMs - t) / 1000));
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
}

/** Parses `eventlog --json` output: one JSON object per line. Bad lines are skipped. */
export function parseRows(stdout) {
  const out = [];
  for (const line of String(stdout ?? "").split("\n")) {
    const s = line.trim();
    if (!s.startsWith("{")) continue;
    try { out.push(JSON.parse(s)); } catch {}
  }
  return out;
}

/** Filters: {types: Set|null ("other" allowed), showQuiet, agent, query}. */
export function matches(e, f) {
  if (!f.showQuiet && QUIET_TYPES.has(e.type) && !(f.types && f.types.has(e.type))) return false;
  if (f.types && f.types.size) {
    const bucket = MAIN_TYPES.includes(e.type) ? e.type : QUIET_TYPES.has(e.type) ? e.type : "other";
    if (!f.types.has(bucket)) return false;
  }
  if (f.agent && actor(e) !== f.agent) return false;
  if (f.query) {
    const q = f.query.toLowerCase();
    const hay = `${e.seq} ${e.type} ${actor(e)} ${headline(e)} ${e.ref ?? ""}`.toLowerCase();
    if (!hay.includes(q)) return false;
  }
  return true;
}

/**
 * Where an event's `ref` points.
 * - {kind: "file", path}: an absolute path, or one relative to the log's project root.
 * - {kind: "remote", host, path}: `host:path`, a file on another machine.
 * - {kind: "none"}
 * A trailing `#anchor` or `:line` is dropped from the path and kept as `anchor`.
 */
export function refTarget(ref, root) {
  if (!ref || typeof ref !== "string") return { kind: "none" };
  let r = ref.trim();
  let anchor = null;
  const hash = r.indexOf("#");
  if (hash > 0) { anchor = r.slice(hash + 1); r = r.slice(0, hash); }
  const remote = /^([A-Za-z0-9._-]+):(?!\/\/)(.+)$/.exec(r);
  if (remote && !r.startsWith("/")) return { kind: "remote", host: remote[1], path: remote[2], anchor };
  if (r.startsWith("~/")) return { kind: "file", path: r, anchor, home: true };
  if (r.startsWith("/")) return { kind: "file", path: r, anchor };
  return { kind: "file", path: `${root.replace(/\/+$/, "")}/${r.replace(/^\.\//, "")}`, anchor };
}

export const MAX_REF_CHARS = 200_000;

/** Markdown files render as markdown; anything else shows as plain text. */
export function isMarkdown(path) {
  return /\.(md|markdown)$/i.test(path);
}

/** Cuts a long file so the panel stays fast, and says so. */
export function clip(text) {
  if (text.length <= MAX_REF_CHARS) return { text, clipped: false };
  return { text: text.slice(0, MAX_REF_CHARS), clipped: true };
}

/** Lifecycle order for the agents list. */
export const PHASES = ["spawned", "claimed", "prompted", "progressing", "resulted", "retired"];

/** Newest agents first; retired ones last unless `showRetired`. */
export function sortAgents(list, showRetired) {
  return list
    .filter((a) => a.agent && (showRetired || a.phase !== "retired"))
    .sort((a, b) => (b.spawned ?? 0) - (a.spawned ?? 0));
}
