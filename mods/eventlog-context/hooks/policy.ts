import type { SessionMessage } from 'claude-code'

/** Marks a compaction this mod asked for; the text after it is the reason. */
export const REBUILD_PREFIX = 'eventlog-rebuild:'

/**
 * The first line of every packet this mod installs. It marks the packet, so
 * a later rebuild never keeps an earlier packet in its tail.
 */
export const PACKET_HEADING = '# Context rebuilt from the event log'

/** The coordination log, relative to the repo root the mod is installed in. */
export const LOG_FILE = '.context/events.jsonl'
export const LAYOUT_FILE = '.context/layout.json'

/** How many per-turn increases the growth estimate averages. */
export const GROWTH_TURNS = 5

export type Packet = { as_of: number; markdown: string; settings: { keep_turns: number; tail_chars: number } }
export type Verdict = { rebuild: boolean; reason: string }
type Run = { exitCode: number; stdout: string }

/** One context reading: tokens, window and percent from `context`, threshold from its breakdown. */
export type Reading = { tokens?: number; window?: number; percent?: number; threshold?: number }
/** The fill in whole percent, and the token limit it was measured against. */
export type Fill = { percent: number; limit: number }

function json(run: Run): unknown {
  if (run.exitCode !== 0 || run.stdout.trim() === '') return null
  try {
    return JSON.parse(run.stdout)
  } catch {
    return null
  }
}

const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v)

/**
 * The `eventlog context --json` output, read three ways: a packet; `error`
 * when eventlog failed or printed nothing usable (the engine compacts and
 * the mod records a fallback); `refused` when the packet is valid JSON but
 * not one to install:
 * - `version`: the packet's `v` is not 1, for example after an `eventlog`
 *   upgrade with no reinstall. The mod falls back *and appends*, so `check`
 *   does not find the same boundary again on every turn.
 * - `empty`: `as_of` is 0 or less (no events yet). The mod falls back
 *   without appending, so it never writes a log that did not exist.
 */
export function readPacket(run: Run): { packet: Packet } | { refused: 'version' | 'empty'; detail: string } | { error: string } {
  const error = { error: 'eventlog context failed' }
  const v = json(run) as (Partial<Packet> & { v?: unknown }) | null
  if (!v || typeof v !== 'object') return error
  if (typeof v.markdown !== 'string' || v.markdown.trim() === '') return error
  if (!isNum(v.as_of) || !v.settings) return error
  if (!isNum(v.settings.keep_turns) || !isNum(v.settings.tail_chars)) return error
  if (v.v !== 1) return { refused: 'version', detail: `packet version ${String(v.v)}` }
  if (v.as_of <= 0) return { refused: 'empty', detail: `no events (as_of ${v.as_of})` }
  return { packet: { as_of: v.as_of, markdown: v.markdown, settings: { keep_turns: v.settings.keep_turns, tail_chars: v.settings.tail_chars } } }
}

export function parsePacket(run: Run): Packet | null {
  const r = readPacket(run)
  return 'packet' in r ? r.packet : null
}

export function parseVerdict(run: Run): Verdict | null {
  const v = json(run) as Partial<Verdict> | null
  if (!v || typeof v !== 'object') return null
  if (typeof v.rebuild !== 'boolean' || typeof v.reason !== 'string') return null
  return { rebuild: v.rebuild, reason: v.reason }
}

/** A packet this mod installed: a user message whose first line is the heading. */
export function isPacket(m: SessionMessage): boolean {
  return m.role === 'user' && (m.text === PACKET_HEADING || m.text.startsWith(`${PACKET_HEADING}\n`))
}

/** The packet text, led by the heading so `isPacket` knows it later. */
export function markPacket(markdown: string): string {
  return isPacket({ role: 'user', text: markdown, toolUses: [] }) ? markdown : `${PACKET_HEADING}\n\n${markdown}`
}

/** A turn starts at a user message that is neither a tool result nor a packet. */
export function isTurnStart(m: SessionMessage): boolean {
  return m.role === 'user' && !(m.toolResults && m.toolResults.length > 0) && !isPacket(m)
}

/** Characters (code points), not UTF-16 units, to match the Rust side's budgets. */
function chars(s: string): number {
  let n = 0
  for (const _ of s) n++
  return n
}

/**
 * What the model reads of a message, counted once: its text, each tool call's
 * name and input, and each tool result's text. A tool's output also appears in
 * `toolUses[].text` and both `result` records; counting those too overcounts
 * it about fourfold (live trim test).
 */
function size(m: SessionMessage): number {
  const uses = (m.toolUses ?? []).reduce((n, u) => n + chars(u.tool) + chars(JSON.stringify(u.input ?? {})), 0)
  const results = (m.toolResults ?? []).reduce((n, r) => n + chars(r.text ?? ''), 0)
  return chars(m.text) + uses + results
}

/** Split into whole turns, oldest first, dropping any earlier packet. */
function turnsOf(all: readonly SessionMessage[]): SessionMessage[][] {
  const messages = all.filter(m => !isPacket(m))
  const starts: number[] = []
  messages.forEach((m, i) => {
    if (isTurnStart(m)) starts.push(i)
  })
  return starts.map((s, i) => messages.slice(s, starts[i + 1] ?? messages.length))
}

const turnSize = (t: SessionMessage[]): number => t.reduce((k, m) => k + size(m), 0)

/**
 * The last `keepTurns` turns, whole, oldest first. Drops whole turns from the
 * oldest end until the tail fits `tailChars`; always keeps the newest turn.
 * When the newest turn alone is over `tailChars`, keeps it trimmed: see
 * `trimTurn`.
 */
export function selectTail(all: readonly SessionMessage[], keepTurns: number, tailChars: number): SessionMessage[] {
  const turns = turnsOf(all)
  if (turns.length === 0 || keepTurns <= 0) return []
  if (newestTurnTrimmed(all, tailChars)) return trimTurn(turns[turns.length - 1], tailChars)
  let kept = turns.slice(-keepTurns)
  const total = (ts: SessionMessage[][]) => ts.reduce((n, t) => n + turnSize(t), 0)
  while (kept.length > 1 && total(kept) > tailChars) kept = kept.slice(1)
  return kept.flat()
}

/**
 * An oversized turn cut down to its prompt and its final answer: the last
 * message when it is assistant text with no tool calls, and both fit
 * `tailChars`. The tool loop between them goes; the packet carries the state.
 * Without the answer the model sees an unanswered prompt and redoes the turn
 * (live trim test), so the answer is kept whenever it fits. A turn cut off
 * mid-loop has no answer; the prompt alone lets the model carry on.
 */
export function trimTurn(turn: readonly SessionMessage[], tailChars: number): SessionMessage[] {
  const prompt = turn[0]
  const last = turn[turn.length - 1]
  const answered = turn.length > 1 && last.role === 'assistant' && last.toolUses.length === 0 && last.text !== ''
  return answered && size(prompt) + size(last) <= tailChars ? [prompt, last] : [prompt]
}

/** Size in characters of the newest turn, as the tail budget counts it; 0 with no turn. */
export function newestTurnSize(all: readonly SessionMessage[]): number {
  const turns = turnsOf(all)
  const newest = turns[turns.length - 1]
  return newest === undefined ? 0 : turnSize(newest)
}

/**
 * True when the newest turn alone is larger than `tailChars` but its prompt
 * fits: the rebuild keeps the prompt and drops the rest of that turn. A
 * newest turn exactly at `tailChars` is kept whole.
 */
export function newestTurnTrimmed(all: readonly SessionMessage[], tailChars: number): boolean {
  const turns = turnsOf(all)
  const newest = turns[turns.length - 1]
  return newest !== undefined && turnSize(newest) > tailChars && size(newest[0]) <= tailChars
}

/**
 * True only when the newest turn's prompt alone is larger than `tailChars`:
 * trimming cannot shrink the context enough, so the mod falls back to
 * Claude Code's own compaction (I1).
 */
export function tailTooLarge(all: readonly SessionMessage[], tailChars: number): boolean {
  const turns = turnsOf(all)
  const newest = turns[turns.length - 1]
  return newest !== undefined && size(newest[0]) > tailChars
}

/**
 * The `rebuild` event's trigger: the engine's, or the reason this mod gave.
 * A plugin's compaction may arrive without a trigger (the test kit passes a
 * plugin call's arguments as given); that reads as `plugin`.
 */
export function triggerOf(trigger: string | undefined, instructions?: string): string {
  const t = trigger ?? 'plugin'
  if (t === 'plugin' && instructions?.startsWith(REBUILD_PREFIX)) {
    const reason = instructions.slice(REBUILD_PREFIX.length).trim()
    if (reason !== '') return reason
  }
  return t
}

export function rebuildArgs(trigger: string, fields: Record<string, string | number>): string[] {
  return ['eventlog', 'append', 'rebuild', `trigger=${trigger}`, ...Object.entries(fields).map(([k, v]) => `${k}=${v}`)]
}

export function checkArgs(percent: number, growth: number): string[] {
  return ['eventlog', 'context', 'check', '--percent', String(percent), '--growth', String(growth)]
}

const clampPercent = (n: number): number => Math.min(100, Math.max(0, Math.round(n)))

/**
 * The fill: context tokens over the auto-compact threshold when both exist;
 * else the engine's `percent` against the window; else null (no reading).
 * A missing reading is never 0.
 */
export function fillOf(r: Reading): Fill | null {
  if (isNum(r.tokens) && isNum(r.threshold) && r.threshold > 0) {
    return { percent: clampPercent((r.tokens / r.threshold) * 100), limit: r.threshold }
  }
  if (isNum(r.percent)) return { percent: clampPercent(r.percent), limit: isNum(r.window) ? r.window : 0 }
  return null
}

/**
 * The history after one more main-loop turn: the increase from `before` to
 * `after` when both exist and it is positive; the newest `GROWTH_TURNS` kept.
 */
export function pushIncrease(history: readonly number[], before: number | undefined, after: number | undefined): number[] {
  if (!isNum(before) || !isNum(after) || after <= before) return [...history]
  return [...history, after - before].slice(-GROWTH_TURNS)
}

/** The average increase as a whole percent of `limit`; 0 with no history or limit. */
export function growthOf(history: readonly number[], limit: number): number {
  if (history.length === 0 || !(limit > 0)) return 0
  const avg = history.reduce((a, b) => a + b, 0) / history.length
  return clampPercent((avg / limit) * 100)
}

/**
 * The repo root when the mod is installed at `<root>/.claude/skills/<name>/`;
 * null anywhere else (for example a `--plugin-dir` checkout).
 */
export function installRoot(pluginRoot: string): string | null {
  const m = /^(.+)\/\.claude\/skills\/[^/]+\/?$/.exec(pluginRoot)
  return m ? m[1] : null
}

export type SessionFacts = {
  isInteractive: boolean
  writer: string | undefined
  setting: string | undefined
  hasLog: boolean
  /** LOG_DRIVEN_WORKER: set in a worker launched by a reactor. */
  worker?: string
  /** HERDR_PANE_ID of this session, and the controller's pane from .context/layout.json. */
  pane?: string
  controllerPane?: string
}

/** The controller's pane from the text of `.context/layout.json`; undefined when absent or unreadable. */
export function controllerPaneOf(layout: string | undefined): string | undefined {
  if (layout === undefined) return undefined
  try {
    const pane = (JSON.parse(layout) as { controller?: { pane?: unknown } }).controller?.pane
    return typeof pane === 'string' && pane !== '' ? pane : undefined
  } catch {
    return undefined
  }
}

/**
 * Why the mod stays out of this session, or null when it acts. It acts only
 * in the controller's interactive session, in a repo that has a log.
 */
export function inertReason(f: SessionFacts): string | null {
  if (!f.isInteractive) return 'headless'
  if (f.writer && f.writer !== 'controller') return `EVENTLOG_AS=${f.writer}`
  if (f.worker) return `LOG_DRIVEN_WORKER=${f.worker}`
  // Same rule as the controller stop hook: once the layout records the
  // controller's pane, a session in any other pane is a worker.
  if (f.pane && f.controllerPane && f.pane !== f.controllerPane) return `pane ${f.pane} is not the controller's pane`
  if (f.setting?.trim().toLowerCase() === 'off') return 'EVENTLOG_CONTEXT=off'
  if (!f.hasLog) return 'no log'
  return null
}
