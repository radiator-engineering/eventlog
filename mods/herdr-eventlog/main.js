// eventlog panel for Herdr: the log's feed, agents, decisions and health, with each
// event's ref (a handoff, a report) readable in place. It only runs the `eventlog`
// command with --json; it never reads or writes the log file itself.
import { View, div } from "gpui-kit";
import { Input, InputState, Scrollbar, TextView, h_flex, v_flex, v_virtual_list } from "gpui-base";
import { closeOverlay, exec, focused, home, openOverlay, props, readTextFile, run, settings, spaces, theme } from "herdr";
import {
  MAIN_TYPES, PLUMBING, actor, ago, clip, headline, isMarkdown, matches, parseRows, refTarget, sortAgents,
} from "./model.js";

const POLL_MS = 3000;
const SLOW_EVERY = 5; // agents and state refresh every 5th poll
const KEEP = 4000; // events kept in memory
const FIRST_LOAD = 1500;
const LIST_CAP = 400;
const ROW_H = 44;
const MAX_TEXT_LINES = 3000;

function typeColor(t, type) {
  switch (type) {
    case "result": return t.accent;
    case "decision": return t.ansi5;
    case "spawn": return t.ansi4;
    case "progress": return t.working;
    case "escalate": case "violation": case "veto": return t.blocked;
    case "message": case "prompt": return t.ansi6;
    case "note": return t.foreground;
    default: return t.dim;
  }
}

function phaseColor(t, phase) {
  return { spawned: t.ansi4, claimed: t.ansi4, prompted: t.ansi6, progressing: t.working, resulted: t.accent, retired: t.dim }[phase] ?? t.dim;
}

const baseName = (p) => (p ?? "").replace(/\/+$/, "").split("/").pop() || p;
const parentOf = (d) => {
  const s = d.replace(/\/+$/, "");
  const i = s.lastIndexOf("/");
  return i <= 0 ? (s === "/" || s === "" ? null : "/") : s.slice(0, i);
};

export default class EventLog extends View {
  init(_props, cx) {
    const p = props() ?? {};
    this.fileMode = typeof p.file === "string" ? p.file : null;
    this.fixedRoot = p.root ?? null;
    this.machine = p.machine ?? null;
    this.overlay = p.overlay ?? null;
    this.tab = "feed";
    this.rows = []; // newest first
    this.lastSeq = 0;
    this.root = null;
    this.dir = null;
    this.ok = false;
    this.error = null;
    this.agents = [];
    this.st = null;
    this.showRetired = false;
    this.filters = { types: new Set(), showQuiet: false, agent: null, query: "" };
    this.detail = null; // an event
    this.ref = null; // {path, status, text, md, clipped, entries, error}
    this.tick = 0;
    this.now = Date.now();
    this.handle = cx.focus_handle();
    // List and scrollbar ids must differ between copies shown at once.
    this.uid = Math.random().toString(36).slice(2, 8);
    this.search = InputState.new({ placeholder: "Search seq, agent, text, ref" });
    this.search.on("change", (_e, cx2) => { this.filters.query = this.search.value(); cx2.notify(); });

    if (this.fileMode) {
      this.openRef(cx, this.fileMode);
      return;
    }
    this.poll(cx);
    cx.timer.every(POLL_MS, (cx2) => this.poll(cx2));
    // A layout or pane can open straight to one event: props {seq}.
    if (Number.isInteger(p.seq)) this.pendingSeq = p.seq;
  }

  // ---------- data ----------

  where() {
    if (this.fixedRoot) return { dir: this.fixedRoot, machine: this.machine };
    const f = focused();
    const space = spaces().find((s) => s.id === f?.space) ?? spaces().find((s) => s.active);
    return { dir: f?.cwd || space?.folder || home(), machine: space?.machine ?? null };
  }

  async findRoot(dir, machine) {
    for (let d = dir, i = 0; d && i < 32; d = parentOf(d), i++) {
      const hit = await readTextFile(`${d === "/" ? "" : d}/.context/eventlog.toml`, { on: machine }).catch(() => null);
      if (hit !== null) return d;
    }
    return null;
  }

  async el(args) {
    const r = await exec("eventlog", ["--json", ...args], { cwd: this.root, on: this.machine });
    if (r.code !== 0) throw new Error((r.stderr || r.stdout || `eventlog exited ${r.code}`).trim().slice(0, 300));
    return r.stdout;
  }

  poll(cx) {
    cx.spawn(async (cx2) => {
      try {
        this.now = Date.now();
        const w = this.where();
        if (w.dir !== this.dir || w.machine !== this.machine || !this.root) {
          // The focused folder first; else the last log this panel showed, so moving to a
          // terminal outside any project keeps the log on screen.
          let root = await this.findRoot(w.dir, w.machine);
          if (!root && !this.fixedRoot) {
            let last = null;
            try { last = JSON.parse(localStorage.getItem("lastRoot") ?? "null"); } catch {}
            if (last?.root) { root = last.root; w.machine = last.machine ?? null; }
          }
          if (root) { try { localStorage.setItem("lastRoot", JSON.stringify({ root, machine: w.machine })); } catch {} }
          this.dir = w.dir;
          if (root !== this.root || w.machine !== this.machine) {
            this.root = root;
            this.machine = w.machine;
            this.rows = [];
            this.lastSeq = 0;
            this.agents = [];
            this.st = null;
            this.tick = 0;
          }
        }
        if (!this.root) { this.ok = false; this.error = null; cx2.notify(); return; }
        const fresh = parseRows(this.lastSeq
          ? await this.el(["view", "--since", String(this.lastSeq + 1)])
          : await this.el(["view", "--last", String(FIRST_LOAD)]));
        if (fresh.length) {
          fresh.sort((a, b) => a.seq - b.seq);
          this.lastSeq = fresh[fresh.length - 1].seq;
          this.rows = fresh.reverse().concat(this.rows).slice(0, KEEP);
        }
        if (this.tick % SLOW_EVERY === 0) {
          this.agents = parseRows(await this.el(["agents"]));
          this.st = JSON.parse(await this.el(["state"]));
        }
        this.tick += 1;
        if (this.pendingSeq !== undefined) { const seq = this.pendingSeq; delete this.pendingSeq; this.openDetailSeq(cx2, seq); }
        this.ok = true;
        this.error = null;
      } catch (err) {
        this.ok = false;
        const msg = String(err?.message ?? err);
        this.error = /not found|no such file|enoent|cannot start/i.test(msg) ? "eventlog is not on PATH" : msg;
      }
      cx2.notify();
    });
  }

  /** Finds one event by seq: from memory, else from the log. */
  async eventAt(seq) {
    const hit = this.rows.find((e) => e.seq === seq);
    if (hit) return hit;
    const n = Number(seq);
    if (!Number.isInteger(n) || n < 0) return null;
    const r = await exec("sh", ["-c", `eventlog --json view --since ${n} | head -n 1`], { cwd: this.root, on: this.machine });
    return parseRows(r.stdout).find((e) => e.seq === n) ?? null;
  }

  openDetail(cx, e) {
    this.detail = e;
    this.ref = null;
    const target = refTarget(e.ref, this.root ?? "");
    if (target.kind === "file") this.openRef(cx, target.home ? `${home()}${target.path.slice(1)}` : target.path);
    else if (target.kind === "remote") this.ref = { path: e.ref, status: "remote", host: target.host };
    cx.notify();
  }

  openDetailSeq(cx, seq) {
    cx.spawn(async (cx2) => {
      const e = await this.eventAt(seq);
      if (e) this.openDetail(cx2, e);
      cx2.notify();
    });
  }

  openRef(cx, path) {
    this.ref = { path, status: "loading" };
    cx.spawn(async (cx2) => {
      const text = await readTextFile(path, { on: this.machine }).catch(() => null);
      if (this.ref?.path !== path) return;
      if (text !== null) {
        const c = clip(text);
        this.ref = { path, status: "ready", md: isMarkdown(path), text: c.text, clipped: c.clipped };
      } else {
        // Not a readable file: a folder lists its entries, anything else is missing.
        const r = await exec("ls", ["-1p", path], { on: this.machine }).catch(() => null);
        if (this.ref?.path !== path) return;
        if (r && r.code === 0) {
          this.ref = { path, status: "folder", entries: r.stdout.split("\n").filter(Boolean).slice(0, 500) };
        } else {
          this.ref = { path, status: "missing" };
        }
      }
      cx2.notify();
    });
  }

  // ---------- UI pieces ----------

  chip(id, label, on, onClick, color) {
    const t = theme();
    return h_flex().id(id).h(22).px(8).gap(5).items_center().rounded(11).border(1).flex_none().text_size(11)
      .border_color(on ? (color ?? t.accent) : t.border).bg(on ? t.selected : t.surface)
      .text_color(on ? t.foreground : t.muted).cursor_pointer().hover((s) => s.bg(t.hover))
      .on_click((_e, cx) => { onClick(); cx.notify(); })
      .when(Boolean(color), (el) => el.child(div().size(6).rounded(3).bg(color)))
      .child(label);
  }

  button(id, label, onClick) {
    const t = theme();
    return h_flex().id(id).h(24).px(9).items_center().rounded(6).border(1).border_color(t.border).flex_none()
      .text_size(11).text_color(t.foreground).cursor_pointer().hover((s) => s.bg(t.hover))
      .on_click((_e, cx) => { onClick(cx); cx.notify(); }).child(label);
  }

  /** Closes this copy when it is shown as a window, popup or dialog. A window has no close of its own. */
  closeButton() {
    const t = theme();
    return h_flex().id("close").size(24).items_center().justify_center().rounded(6).flex_none()
      .text_size(13).text_color(t.muted).cursor_pointer().hover((s) => s.bg(t.hover).text_color(t.foreground))
      .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
      .on_click(() => closeOverlay(this.overlay)).child("✕");
  }

  /** A virtual list that fills the rest of its column, with a scrollbar. */
  vlist(name, items, rowH, key, row) {
    // A plain scroll column: v_virtual_list drew no rows in the dock panel.
    const shown = items.slice(0, LIST_CAP);
    return v_flex().id(`${name}-${this.uid}`).flex_1().min_h(0).overflow_y_scrollbar()
      .children(shown.map(row))
      .when(items.length > shown.length, (el) => el.child(div().p(10).text_size(11).text_color(theme().dim)
        .child(`Showing the newest ${shown.length}; search or filter to narrow.`)));
  }

  header() {
    const t = theme();
    const live = this.ok ? t.done : this.error ? t.blocked : t.dim;
    return h_flex().flex_none().h(34).px(10).gap(8).items_center().border_b(1).border_color(t.border)
      .child(div().size(7).rounded(4).bg(live))
      .child(div().text_size(13).font_semibold().text_color(t.foreground).child("Log"))
      .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(t.dim)
        .child(this.root ? `${baseName(this.root)}${this.machine ? ` on ${this.machine}` : ""} · #${this.lastSeq}` : ""))
      .when(Boolean(this.root) && !this.overlay, (el) => el.child(this.button("pop", "Full view", () => {
        openOverlay({ mod: "eventlog", kind: "popup", width: 0.92, height: 0.88, props: { root: this.root, machine: this.machine } });
      })))
      .when(Boolean(this.overlay), (el) => el.child(this.closeButton()));
  }

  tabs() {
    const t = theme();
    const st = this.st;
    const active = this.agents.filter((a) => a.agent && a.phase !== "retired").length;
    const esc = st?.open_escalations?.length ?? 0;
    const behind = (st?.reactors ?? []).filter((r) => (r.unacked?.length ?? 0) > 0).length;
    const tab = (id, label, count, warn) => {
      const on = this.tab === id && !this.detail;
      return h_flex().id(`tab-${id}`).h(30).px(7).gap(5).items_center().cursor_pointer()
        .border_b(2).border_color(on ? t.accent : t.chrome)
        .text_size(12).text_color(on ? t.foreground : t.muted).hover((s) => s.text_color(t.foreground))
        .on_click((_e, cx) => { this.tab = id; this.detail = null; cx.notify(); })
        .child(label)
        .when(count !== undefined && count !== null, (el) => el.child(
          div().px(5).rounded(8).text_size(10).bg(warn ? t.blocked : t.raised).text_color(warn ? t.solid_foreground : t.dim).child(String(count))));
    };
    return h_flex().flex_none().px(4).border_b(1).border_color(t.border).bg(t.chrome)
      .child(tab("feed", "Feed"))
      .child(tab("agents", "Agents", active))
      .child(tab("decisions", "Decisions", st?.decisions?.length))
      .child(tab("health", "Health", esc + behind || null, esc > 0));
  }

  searchBox() {
    const t = theme();
    return div().id("search").flex_1().min_w(0).h(28).px(8).flex().items_center().rounded(6).border(1)
      .border_color(t.input).bg(t.background).cursor_text()
      .on_mouse_down("left", (_e, cx) => cx.stop_propagation())
      .child(Input.new(this.search).h_full().text_size(12).text_color(t.foreground));
  }

  feedRow(e) {
    const t = theme();
    const who = actor(e);
    return h_flex().id(`ev-${e.seq}`).w_full().h(ROW_H).px(10).gap(8).items_center().border_b(1).border_color(t.border)
      .cursor_pointer().hover((s) => s.bg(t.hover))
      .on_click((_e, cx) => this.openDetail(cx, e))
      .child(div().w(62).flex_none().truncate().text_size(11).font_medium().text_color(typeColor(t, e.type)).child(e.type))
      .child(v_flex().flex_1().min_w(0).gap(2)
        .child(div().w_full().truncate().text_size(12).text_color(t.foreground).child(headline(e) || "—"))
        .child(h_flex().gap(6).text_size(10).text_color(t.dim).min_w(0)
          .child(div().flex_none().child(`#${e.seq}`))
          .child(div().flex_none().child(ago(e.ts, this.now)))
          .when(Boolean(who), (el) => el.child(div().min_w(0).truncate().child(who)))
          .when(Boolean(e.ref), (el) => el.child(div().flex_none().text_color(t.muted).child("· has file")))));
  }

  feed() {
    const t = theme();
    const f = this.filters;
    const list = this.rows.filter((e) => matches(e, f));
    const toggle = (type) => { if (f.types.has(type)) f.types.delete(type); else f.types.add(type); };
    const chips = h_flex().flex_none().flex_wrap().gap(4).px(10).pb(8)
      .child(this.chip("c-all", "All", f.types.size === 0, () => f.types.clear()))
      .children([...MAIN_TYPES, "other"].map((ty) => this.chip(`c-${ty}`, ty, f.types.has(ty), () => toggle(ty), typeColor(t, ty))))
      .child(this.chip("c-quiet", "rebuilds", f.showQuiet, () => { f.showQuiet = !f.showQuiet; }))
      .when(Boolean(f.agent), (el) => el.child(this.chip("c-agent", `${f.agent}  ✕`, true, () => { f.agent = null; })));
    return v_flex().flex_1().min_h(0)
      .child(h_flex().flex_none().p(10).pb(6).child(this.searchBox()))
      .child(chips)
      .child(div().flex_none().px(10).pb(4).text_size(10).text_color(t.dim)
        .child(`${list.length} of the last ${this.rows.length} events`))
      .child(this.vlist("feed", list, ROW_H, (e) => String(e.seq), (e) => this.feedRow(e)))
      .when(list.length === 0, (el) => el.child(div().p(16).text_color(t.dim).text_size(12)
        .child(this.rows.length ? "No event matches these filters." : "No events yet.")));
  }

  agentsTab() {
    const t = theme();
    const list = sortAgents(this.agents, this.showRetired);
    const retired = this.agents.filter((a) => a.phase === "retired").length;
    return v_flex().flex_1().min_h(0)
      .child(h_flex().flex_none().p(10).gap(6)
        .child(this.chip("c-retired", `Show retired (${retired})`, this.showRetired, () => { this.showRetired = !this.showRetired; })))
      .child(this.vlist("agents", list, ROW_H, (a) => a.agent, (a) =>
            h_flex().id(`ag-${a.agent}`).w_full().h(ROW_H).px(10).gap(8).items_center().border_b(1).border_color(t.border)
              .cursor_pointer().hover((s) => s.bg(t.hover))
              .on_click((_e, cx) => { this.filters.agent = a.agent; this.tab = "feed"; cx.notify(); })
              .child(div().size(8).rounded(4).flex_none().bg(phaseColor(t, a.phase)))
              .child(v_flex().flex_1().min_w(0).gap(2)
                .child(div().truncate().text_size(12).text_color(t.foreground).child(a.agent))
                .child(h_flex().gap(6).text_size(10).text_color(t.dim).min_w(0)
                  .child(div().flex_none().text_color(phaseColor(t, a.phase)).child(a.phase))
                  .when(Boolean(a.model), (el) => el.child(div().flex_none().child(a.model)))
                  .child(div().flex_none().child(`#${a.spawned}`))
                  .when(Boolean(a.claims), (el) => el.child(div().min_w(0).truncate().child(a.claims)))))));
  }

  decisionsTab() {
    const t = theme();
    const q = this.filters.query.toLowerCase();
    const list = [...(this.st?.decisions ?? [])]
      .filter((d) => !q || `${d.key} ${d.value}`.toLowerCase().includes(q))
      .sort((a, b) => b.seq - a.seq);
    return v_flex().flex_1().min_h(0)
      .child(h_flex().flex_none().p(10).pb(6).child(this.searchBox()))
      .child(div().flex_none().px(10).pb(4).text_size(10).text_color(t.dim).child(`${list.length} decisions in force, newest first`))
      .child(this.vlist("decisions", list, 52, (d) => d.key, (d) =>
            v_flex().id(`dec-${d.key}`).w_full().h(52).px(10).py(6).gap(2).border_b(1).border_color(t.border)
              .cursor_pointer().hover((s) => s.bg(t.hover))
              .on_click((_e, cx) => this.openDetailSeq(cx, d.seq))
              .child(h_flex().gap(6).min_w(0)
                .child(div().min_w(0).truncate().text_size(12).font_medium().text_color(t.ansi5).child(d.key))
                .child(div().flex_none().text_size(10).text_color(t.dim).child(`#${d.seq}`)))
              .child(div().w_full().truncate().text_size(11).text_color(t.muted).child(String(d.value ?? "")))));
  }

  section(title, items, empty) {
    const t = theme();
    return v_flex().gap(4)
      .child(div().text_size(10).font_semibold().text_color(t.dim).child(title.toUpperCase()))
      .children(items.length ? items : [div().text_size(11).text_color(t.dim).child(empty)]);
  }

  healthTab() {
    const t = theme();
    const st = this.st ?? {};
    const line = (id, color, main, sub, onClick) => h_flex().id(id).gap(8).py(4).items_start().rounded(4)
      .when(Boolean(onClick), (el) => el.cursor_pointer().hover((s) => s.bg(t.hover)).on_click((_e, cx) => { onClick(); cx.notify(); }))
      .child(div().mt(4).size(7).rounded(4).flex_none().bg(color))
      .child(v_flex().flex_1().min_w(0)
        .child(div().text_size(12).text_color(t.foreground).child(main))
        .when(Boolean(sub), (el) => el.child(div().text_size(11).text_color(t.muted).child(sub))));
    const toAgent = (a) => () => { this.filters.agent = a; this.tab = "feed"; };
    return v_flex().id("health").flex_1().min_h(0).p(12).gap(14).overflow_y_scrollbar()
      .child(this.section("Escalations open", (st.open_escalations ?? []).map((e, i) =>
        line(`esc-${i}`, t.blocked, e.agent || "?", e.msg, toAgent(e.agent))), "None"))
      .child(this.section("Reactors", (st.reactors ?? []).map((r) => {
        const n = r.unacked?.length ?? 0;
        return line(`re-${r.name}`, n ? t.blocked : t.done, `${r.name}: ${n ? `${n} not acknowledged` : "up to date"}`,
          `last ack #${r.last_ack_seq ?? "?"} · ${r.age ?? "?"} ago`);
      }), "None"))
      .child(this.section("Intents open", (st.open_intents ?? []).map((e, i) =>
        line(`int-${i}`, t.working, e.agent || "?", e.msg, toAgent(e.agent))), "None"))
      .child(this.section("Claims open", (st.open_claims ?? []).map((c, i) =>
        line(`cl-${i}`, t.ansi4, c.agent, c.path, toAgent(c.agent))), "None"));
  }

  refView() {
    const t = theme();
    const r = this.ref;
    if (!r) return null;
    const mono = settings().font_family ?? "JetBrains Mono";
    const top = h_flex().gap(6).items_center().min_w(0)
      .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(t.muted).child(r.path))
      .when(r.status === "ready" && !this.fileMode, (el) => el
        .child(this.button("ref-pane", "Open beside", () => run("pane.split", { direction: "right", mod: "eventlog", props: { file: r.path, machine: this.machine } })))
        .child(this.button("ref-win", "Window", () => openOverlay({ mod: "eventlog", kind: "window", width: 760, height: 640, props: { file: r.path, machine: this.machine } }))));
    let body;
    if (r.status === "loading") body = div().text_size(11).text_color(t.dim).child("Loading…");
    else if (r.status === "remote") body = div().text_size(11).text_color(t.dim).child(`This file is on ${r.host}. Open the log from a space on that machine to read it.`);
    else if (r.status === "missing") body = div().text_size(11).text_color(t.dim).child("The file is not there any more.");
    else if (r.status === "folder") {
      body = v_flex().gap(1).children(r.entries.map((name, i) =>
        div().id(`ent-${i}`).px(4).py(2).rounded(4).text_size(12).font_family(mono).text_color(name.endsWith("/") ? t.accent : t.foreground)
          .cursor_pointer().hover((s) => s.bg(t.hover))
          .on_click((_e, cx) => this.openRef(cx, `${r.path.replace(/\/+$/, "")}/${name.replace(/\/$/, "")}`))
          .child(name)));
    } else if (r.md) {
      body = div().text_size(13).text_color(t.foreground).child(TextView.markdown(`md-${r.path}`, r.text));
    } else {
      const lines = r.text.split("\n");
      body = v_flex().font_family(mono).text_size(11).text_color(t.foreground)
        .children(lines.slice(0, MAX_TEXT_LINES).map((l, i) => div().min_h(15).child(l || " ")))
        .when(lines.length > MAX_TEXT_LINES, (el) => el.child(div().pt(6).text_color(t.dim).child(`Showing ${MAX_TEXT_LINES} of ${lines.length} lines.`)));
    }
    return v_flex().gap(8)
      .child(top)
      .child(body)
      .when(Boolean(r.clipped), (el) => el.child(div().text_size(11).text_color(t.dim).child("The file is long; the rest is cut off here.")));
  }

  detailView() {
    const t = theme();
    const e = this.detail;
    const who = actor(e);
    const shown = headline(e);
    const fields = Object.keys(e).filter((k) => !PLUMBING.has(k) && String(e[k]) !== shown);
    const trail = who ? this.rows.filter((x) => actor(x) === who && x.seq !== e.seq).slice(0, 30) : [];
    return v_flex().flex_1().min_h(0)
      .child(h_flex().flex_none().h(36).px(10).gap(8).items_center().border_b(1).border_color(t.border)
        .child(this.button("back", "← Back", () => { this.detail = null; this.ref = null; }))
        .child(div().text_size(12).font_semibold().text_color(typeColor(t, e.type)).child(e.type))
        .child(div().text_size(11).text_color(t.dim).child(`#${e.seq} · ${e.ts?.replace("T", " ").replace("Z", "") ?? ""} · ${ago(e.ts, this.now)} ago`)))
      .child(v_flex().id("detail-scroll").flex_1().min_h(0).p(12).gap(14).overflow_y_scrollbar()
        .child(div().text_size(13).text_color(t.foreground).child(shown || "—"))
        .child(v_flex().gap(3).children(fields.map((k) =>
          h_flex().gap(8).items_start()
            .child(div().w(90).flex_none().text_size(11).text_color(t.dim).child(k))
            .child(div().flex_1().min_w(0).text_size(11).text_color(k === "agent" ? t.accent : t.foreground)
              .when(k === "agent", (el) => el.id("f-agent").cursor_pointer()
                .on_click((_e, cx) => { this.filters.agent = e.agent; this.detail = null; this.tab = "feed"; cx.notify(); }))
              .child(String(e[k]))))))
        .when(Boolean(this.ref), (el) => el.child(v_flex().gap(6)
          .child(div().text_size(10).font_semibold().text_color(t.dim).child("FILE"))
          .child(div().p(10).rounded(6).border(1).border_color(t.border).bg(t.background).child(this.refView()))))
        .when(trail.length > 0, (el) => el.child(v_flex().gap(2)
          .child(div().text_size(10).font_semibold().text_color(t.dim).child(`MORE FROM ${who.toUpperCase()}`))
          .children(trail.map((x) => h_flex().id(`tr-${x.seq}`).gap(8).py(3).px(4).rounded(4).cursor_pointer()
            .hover((s) => s.bg(t.hover)).on_click((_e, cx) => this.openDetail(cx, x))
            .child(div().w(42).flex_none().text_size(10).text_color(t.dim).child(`#${x.seq}`))
            .child(div().w(62).flex_none().text_size(11).text_color(typeColor(t, x.type)).child(x.type))
            .child(div().flex_1().min_w(0).truncate().text_size(11).text_color(t.foreground).child(headline(x))))))));
  }

  render(_cx) {
    const t = theme();
    const root = v_flex().id("root").size_full().bg(t.surface).text_color(t.foreground)
      .track_focus(this.handle)
      .on_key_down((ev, cx) => {
        if (ev.key === "escape" && this.detail) { this.detail = null; this.ref = null; cx.notify(); }
        else if (ev.key === "escape" && this.overlay) { closeOverlay(this.overlay); cx.stop_propagation(); }
      });
    if (this.fileMode) {
      return root
        .child(h_flex().flex_none().h(34).px(10).items_center().border_b(1).border_color(t.border)
          .child(div().flex_1().min_w(0).truncate().text_size(12).font_semibold().child(baseName(this.fileMode)))
          .when(Boolean(this.overlay), (el) => el.child(this.closeButton())))
        .child(v_flex().id("file-scroll").flex_1().min_h(0).p(12).overflow_y_scrollbar().child(this.refView()));
    }
    root.child(this.header());
    if (!this.root) {
      return root.child(v_flex().flex_1().items_center().justify_center().gap(6).p(20)
        .child(div().text_size(13).text_color(t.foreground).child(this.error ?? "No event log here"))
        .child(div().text_size(11).text_color(t.dim).child("Open a terminal in a project that has .context/eventlog.toml.")));
    }
    root.child(this.tabs());
    if (this.error) root.child(div().flex_none().px(10).py(4).text_size(11).text_color(t.blocked).child(this.error));
    if (this.detail) return root.child(this.detailView());
    if (this.tab === "agents") return root.child(this.agentsTab());
    if (this.tab === "decisions") return root.child(this.decisionsTab());
    if (this.tab === "health") return root.child(this.healthTab());
    return root.child(this.feed());
  }
}
