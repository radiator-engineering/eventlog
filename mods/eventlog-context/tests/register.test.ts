import type { CommandRunInput, On, SessionMessage, TurnCompleteInput } from 'claude-code'
import type { Engine } from 'claude-code/testing'
import { describe, expect, mock, test, tier } from 'claude-code/testing'

tier('user')

const PACKET = JSON.stringify({ v: 1, as_of: 42, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 1, tail_chars: 100000 } })
const msgs: SessionMessage[] = [
  { role: 'user', text: 'old', toolUses: [], handle: 'h1' },
  { role: 'assistant', text: 'old answer', toolUses: [], handle: 'h2' },
  { role: 'user', text: 'new', toolUses: [], handle: 'h3' },
  { role: 'assistant', text: 'new answer', toolUses: [], handle: 'h4' },
]

type Answer = { exitCode: number; stdout: string } | 'throw'
type Usage = { tokens?: number; percent?: number; window?: number; threshold?: number }

type AnsweredTurn = Exclude<TurnCompleteInput, { reason: 'refusal' }>
let turns = 0
const turn = (over: Partial<AnsweredTurn> = {}): TurnCompleteInput => ({
  answer: 'ok', durationMs: 1000, isAborted: false, turnId: `t${++turns}`, reason: 'answer', ...over,
})

const rebuildCmd = (): CommandRunInput => ({
  command: 'rebuild', args: '', origin: { kind: 'composer' }, presentation: { isFullscreen: false, columns: 80 },
})

/**
 * The world beneath the mod. `process.run` answers by the argv's second and
 * third words ('context --json', 'context check', 'append rebuild'); the
 * engine's own compaction is a stand-in that records it ran. `usage` is read
 * on each call, so a test can move the context between turns.
 */
type WorldOpts = {
  summarizerThrows?: boolean
  /** Variables the mod reads through $.env. */
  env?: Record<string, string>
  /** Text of .context/layout.json; the file is absent when omitted. */
  layout?: string
  /** Whether the log file exists under the root (default true). */
  hasLog?: boolean
  /** What `git rev-parse --show-toplevel` prints; null for "not a repo". */
  gitRoot?: string | null
}

function world(on: On, answers: Record<string, Answer>, usage: Usage = {}, opts: WorldOpts = {}) {
  const calls: string[][] = []
  const cwds: Array<string | undefined> = []
  const git: string[][] = []
  const exists: string[] = []
  const logs: string[] = []
  const summarized: string[] = []
  const commands: string[] = []
  mock.env(on, opts.env ?? {})
  on('fs.exists', ($, e) => {
    exists.push(e.path)
    if (e.path.endsWith('.context/layout.json')) return { value: opts.layout !== undefined }
    return { value: opts.hasLog ?? true }
  })
  on('fs.read', () => ({ value: opts.layout ?? '' }))
  on('process.run', ($, e) => {
    if (e.argv[0] === 'git') {
      git.push([...e.argv])
      const root = opts.gitRoot === undefined ? '/repo' : opts.gitRoot
      return { value: root === null ? { exitCode: 128, stdout: '', stderr: 'not a git repository' } : { exitCode: 0, stdout: `${root}\n`, stderr: '' } }
    }
    calls.push([...e.argv])
    cwds.push(e.init?.cwd)
    const answer = answers[e.argv.slice(1, 3).join(' ')] ?? { exitCode: 0, stdout: '' }
    if (answer === 'throw') throw new Error('process.run: timed out after 5000 ms')
    return { value: { stderr: 'append failed', ...answer } }
  })
  on('session.usage', ($, e) => {
    const context: Record<string, unknown> = { window: usage.window ?? 200_000, tokens: usage.tokens, percent: usage.percent }
    if (e.breakdown && usage.threshold !== undefined) {
      context.breakdown = { autoCompactThreshold: usage.threshold, isAutoCompactEnabled: true }
    }
    return { value: { startedAt: 0, context, rateLimits: [] } as never }
  })
  on('session.compact', ($, e) => {
    summarized.push(e.trigger)
    if (opts.summarizerThrows) throw new Error('not available in a headless (-p / SDK) session yet')
    return { messages: [msgs[0]] }
  })
  on('session.messages', () => ({ value: msgs }) as never)
  on('turn.complete', ($, e) => ({ text: e.answer }))
  on('session.start', ($, e) => ({ cwd: e.cwd }) as never)
  on('command.register', ($, e) => {
    commands.push(e.name)
    return { value: { command: e.name } }
  })
  on('ui.log', ($, e) => {
    logs.push(e.text)
    return { value: undefined }
  })
  const runs = (words: string) => calls.filter(c => c.slice(1, 3).join(' ') === words)
  /** Starts the session: the mod decides here whether it acts. */
  const start = ($: Engine, isInteractive = true) =>
    $.session.start({ surface: isInteractive ? 'terminal' : null, isInteractive, cwd: '/repo/sub' })
  return { calls, cwds, git, exists, logs, summarized, commands, runs, start }
}

/**
 * An outer plugin that records what the session.compact chain beneath it
 * settles on: the mod's own answer when the mod handles the compaction.
 *
 * The kit raises a plugin's `$.session.compact({ instructions })` with no
 * `messages`, and refuses a `next(e)` that forwards it so. The observer fills
 * them in from the transcript, as the real engine does (spike, Q2).
 * An inline plugin runs in its own environment, so it reports through
 * $.ui.log, which the test's world captures. Nothing outside its register
 * function reaches it, so the prefix is spelled out in the hook.
 */
const OBSERVED = 'observer: '
const observer = {
  name: 'observer',
  tier: 'prepend' as const,
  register: (on: On) => {
    on('session.compact', async ($, e, next) => {
      const r = await next(Array.isArray(e.messages) ? e : { ...e, messages: await $.session.messages() })
      $.ui.log('observer: ' + JSON.stringify(r.messages?.map(m => [m.text, m.handle ?? null]) ?? { skip: r.skip }))
      return r
    })
  },
}

/** An outer plugin that vetoes every compaction. */
const vetoer = {
  name: 'vetoer',
  tier: 'prepend' as const,
  register: (on: On) => {
    on('session.compact', () => ({ skip: 'vetoed by a test plugin' }))
  },
}

const OK = { exitCode: 0, stdout: '' }
const yes = (reason: string) => ({ exitCode: 0, stdout: JSON.stringify({ rebuild: true, reason }) + '\n' })
const no = (reason: string) => ({ exitCode: 0, stdout: JSON.stringify({ rebuild: false, reason }) + '\n' })

describe('which sessions the mod acts in', () => {
  const ALL = { 'context --json': { exitCode: 0, stdout: PACKET }, 'append rebuild': OK, 'context check': yes('boundary') }

  /** An inert mod: every hook passes to next, and no eventlog process runs. */
  async function expectInert($: Engine, w: ReturnType<typeof world>) {
    const r = await $.session.compact({ trigger: 'auto', messages: msgs })
    expect(r.messages?.map(m => m.text)).toEqual(['old'])
    expect(w.summarized).toEqual(['auto'])
    const t = await $.turn.complete(turn())
    expect(t.text).toBe('ok')
    expect(w.commands).toEqual([])
    expect(w.calls).toEqual([])
  }

  test('a headless session is inert and runs nothing, not even git', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 })
    await w.start($, false)
    await expectInert($, w)
    expect(w.git).toEqual([])
    expect(w.logs.filter(l => l.includes('inert'))).toEqual(['eventlog-context: inert in this session (headless)'])
  })

  test('before session.start the mod is inert', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 })
    await expectInert($, w)
  })

  test('EVENTLOG_AS naming another writer is inert', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { env: { EVENTLOG_AS: 'worker-1' } })
    await w.start($)
    await expectInert($, w)
    expect(w.logs.some(l => l.includes('EVENTLOG_AS=worker-1'))).toBe(true)
  })

  test('LOG_DRIVEN_WORKER is inert', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { env: { LOG_DRIVEN_WORKER: 'doc-worker' } })
    await w.start($)
    await expectInert($, w)
    expect(w.logs.some(l => l.includes('LOG_DRIVEN_WORKER=doc-worker'))).toBe(true)
  })

  test("a herdr pane other than the controller's is inert", async ($, on) => {
    const layout = JSON.stringify({ controller: { pane: 'w1:p1' } })
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { env: { HERDR_PANE_ID: 'w9:p1' }, layout })
    await w.start($)
    await expectInert($, w)
    expect(w.logs.some(l => l.includes('not the controller'))).toBe(true)
  })

  test("the controller's own herdr pane acts", async ($, on) => {
    const layout = JSON.stringify({ controller: { pane: 'w1:p1' } })
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { env: { HERDR_PANE_ID: 'w1:p1' }, layout })
    await w.start($)
    expect(w.commands).toEqual(['rebuild'])
    await $.turn.complete(turn())
    expect(w.runs('append rebuild')).toHaveLength(1)
  })

  test('a herdr pane with an unreadable layout file acts', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { env: { HERDR_PANE_ID: 'w9:p1' }, layout: 'not json' })
    await w.start($)
    expect(w.commands).toEqual(['rebuild'])
    await $.turn.complete(turn())
    expect(w.runs('append rebuild')).toHaveLength(1)
  })

  test('EVENTLOG_AS=controller acts', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { env: { EVENTLOG_AS: 'controller' } })
    await w.start($)
    expect(w.commands).toEqual(['rebuild'])
    await $.turn.complete(turn())
    expect(w.runs('append rebuild')).toHaveLength(1)
  })

  test('EVENTLOG_CONTEXT=off is inert', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { env: { EVENTLOG_CONTEXT: 'off' } })
    await w.start($)
    await expectInert($, w)
  })

  test('a repo root without the log is inert and appends nothing', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { hasLog: false })
    await w.start($)
    await expectInert($, w)
    expect(w.exists).toEqual(['/repo/.context/events.jsonl'])
    expect(w.runs('append rebuild')).toEqual([])
  })

  test('outside a git repo the mod is inert', async ($, on) => {
    const w = world(on, ALL, { tokens: 150_000, percent: 75 }, { gitRoot: null })
    await w.start($)
    await expectInert($, w)
    expect(w.exists).toEqual([])
  })
})

describe('session.compact', () => {
  test('returns the packet plus the tail without the summarizer', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: PACKET }, 'append rebuild': OK }, { tokens: 12_345, percent: 6 })
    await w.start($)
    const r = await $.session.compact({ trigger: 'plugin', instructions: 'eventlog-rebuild:boundary', messages: msgs })
    expect(r.messages?.map(m => m.text)).toEqual(['# Context rebuilt from the event log', 'new', 'new answer'])
    expect(r.messages?.[0].role).toBe('user')
    expect(w.summarized).toEqual([])
    expect(w.calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=boundary', 'as_of=42', 'tokens_before=12345', 'kept_turns=1'])
  })

  test('omits tokens_before when usage has no figure', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: PACKET }, 'append rebuild': OK }, {})
    await w.start($)
    await $.session.compact({ trigger: 'auto', messages: msgs })
    expect(w.runs('append rebuild')).toEqual([['eventlog', 'append', 'rebuild', 'trigger=auto', 'as_of=42', 'kept_turns=1']])
  })

  test('manual and foreign plugin compactions keep the engine trigger', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: PACKET }, 'append rebuild': OK })
    await w.start($)
    await $.session.compact({ trigger: 'manual', messages: msgs })
    await $.session.compact({ trigger: 'plugin', instructions: 'keep the API notes', messages: msgs })
    expect(w.runs('append rebuild').map(c => c[3])).toEqual(['trigger=manual', 'trigger=plugin'])
    expect(w.summarized).toEqual([])
  })

  test('fallback: eventlog fails, engine compacts, rebuild event records it', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 1, stdout: '' }, 'append rebuild': OK })
    await w.start($)
    const r = await $.session.compact({ trigger: 'plugin', messages: msgs })
    expect(w.summarized).toEqual(['plugin'])
    expect(r.messages?.map(m => m.text)).toEqual(['old'])
    expect(w.calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=plugin', 'reason=engine-fallback'])
  })

  test('fallback: eventlog times out', async ($, on) => {
    const w = world(on, { 'context --json': 'throw', 'append rebuild': OK })
    await w.start($)
    await $.session.compact({ trigger: 'auto', messages: msgs })
    expect(w.summarized).toEqual(['auto'])
    expect(w.calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=auto', 'reason=engine-fallback'])
  })

  test('fallback: eventlog prints nothing', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: '' }, 'append rebuild': OK })
    await w.start($)
    await $.session.compact({ trigger: 'manual', messages: msgs })
    expect(w.summarized).toEqual(['manual'])
    expect(w.calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=manual', 'reason=engine-fallback'])
  })

  test('fallback: the append failing too still compacts, and says so', async ($, on) => {
    const w = world(on, { 'context --json': 'throw', 'append rebuild': 'throw' })
    await w.start($)
    const r = await $.session.compact({ trigger: 'auto', messages: msgs })
    expect(w.summarized).toEqual(['auto'])
    expect(r.messages?.length).toBe(1)
    expect(w.logs.some(l => l.includes('rebuild event not appended'))).toBe(true)
  })

  test('a failed append keeps the rebuild and logs it', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: PACKET }, 'append rebuild': { exitCode: 2, stdout: '' } })
    await w.start($)
    const r = await $.session.compact({ trigger: 'manual', messages: msgs })
    expect(r.messages?.[0].text).toBe('# Context rebuilt from the event log')
    expect(w.summarized).toEqual([])
    expect(w.logs.some(l => l.includes('rebuild event not appended'))).toBe(true)
  })

  test('precompute is skipped and runs no eventlog process', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: PACKET } })
    await w.start($)
    const r = await $.session.compact({ trigger: 'precompute', messages: msgs })
    expect(r.skip).toContain('eventlog-context')
    expect(w.calls).toEqual([])
    expect(w.summarized).toEqual([])
  })

  test('a packet with no events falls back without appending', async ($, on) => {
    const empty = JSON.stringify({ v: 1, as_of: 0, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 1, tail_chars: 100000 } })
    const w = world(on, { 'context --json': { exitCode: 0, stdout: empty }, 'append rebuild': OK })
    await w.start($)
    const r = await $.session.compact({ trigger: 'auto', messages: msgs })
    expect(w.summarized).toEqual(['auto'])
    expect(r.messages?.map(m => m.text)).toEqual(['old'])
    expect(w.runs('append rebuild')).toEqual([])
  })

  test('a packet of an unknown version falls back and appends reason=engine-fallback (M1)', async ($, on) => {
    // Unlike the empty-log refusal, a version mismatch means eventlog was
    // upgraded without a reinstall: appending nothing would make `check`
    // find the same boundary again on every turn.
    const v2 = JSON.stringify({ v: 2, as_of: 42, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 1, tail_chars: 100000 } })
    const w = world(on, { 'context --json': { exitCode: 0, stdout: v2 }, 'append rebuild': OK })
    await w.start($)
    await $.session.compact({ trigger: 'manual', messages: msgs })
    expect(w.summarized).toEqual(['manual'])
    expect(w.runs('append rebuild')).toEqual([['eventlog', 'append', 'rebuild', 'trigger=manual', 'reason=engine-fallback']])
  })

  test('an earlier packet in the transcript is not kept in the tail', async ($, on) => {
    const three = JSON.stringify({ v: 1, as_of: 42, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 3, tail_chars: 100000 } })
    const w = world(on, { 'context --json': { exitCode: 0, stdout: three }, 'append rebuild': OK })
    await w.start($)
    const oldPacket: SessionMessage = { role: 'user', text: '# Context rebuilt from the event log\n\nolder packet', toolUses: [], handle: 'h0' }
    const r = await $.session.compact({ trigger: 'manual', messages: [oldPacket, msgs[0], msgs[1]] })
    expect(r.messages?.map(m => m.text)).toEqual(['# Context rebuilt from the event log', 'old', 'old answer'])
    expect(w.runs('append rebuild')[0]).toContain('kept_turns=1')
  })

  test('the packet is led by the heading even when eventlog renders another', async ($, on) => {
    const other = JSON.stringify({ v: 1, as_of: 42, markdown: '# Something else', settings: { keep_turns: 1, tail_chars: 100000 } })
    const w = world(on, { 'context --json': { exitCode: 0, stdout: other }, 'append rebuild': OK })
    await w.start($)
    const r = await $.session.compact({ trigger: 'manual', messages: msgs })
    expect(r.messages?.[0].text).toBe('# Context rebuilt from the event log\n\n# Something else')
  })

  test('an oversized newest turn keeps its prompt and drops the rest, reason=tail-trimmed', async ($, on) => {
    // The newest turn ('new' 3 + 'new answer' 10) is bigger than
    // tail_chars=10, and so is prompt plus answer; the prompt alone fits.
    const tiny = JSON.stringify({ v: 1, as_of: 42, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 3, tail_chars: 10 } })
    const w = world(on, { 'context --json': { exitCode: 0, stdout: tiny }, 'append rebuild': OK })
    await w.start($)
    const r = await $.session.compact({ trigger: 'auto', messages: msgs })
    expect(w.summarized).toEqual([])
    expect(r.messages?.map(m => m.text)).toEqual(['# Context rebuilt from the event log', 'new'])
    expect(w.calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=auto', 'as_of=42', 'kept_turns=1', 'reason=tail-trimmed'])
  })

  test('a trimmed turn keeps its final answer and drops the tool loop', async ($, on) => {
    const loop: SessionMessage[] = [
      ...msgs.slice(0, 2),
      { role: 'user', text: 'read it', toolUses: [], handle: 'h5' },
      { role: 'assistant', text: '', toolUses: [{ tool_use_id: 't1', tool: 'Read', input: { file_path: 'a.rs' } }], handle: 'h6' },
      { role: 'user', text: '', toolUses: [], toolResults: [{ tool_use_id: 't1', text: 'x'.repeat(500), isError: false }], handle: 'h7' },
      { role: 'assistant', text: 'it is fine', toolUses: [], handle: 'h8' },
    ]
    const small = JSON.stringify({ v: 1, as_of: 42, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 3, tail_chars: 100 } })
    const w = world(on, { 'context --json': { exitCode: 0, stdout: small }, 'append rebuild': OK })
    await w.start($)
    const r = await $.session.compact({ trigger: 'auto', messages: loop })
    expect(w.summarized).toEqual([])
    expect(r.messages?.map(m => m.handle ?? m.text)).toEqual(['# Context rebuilt from the event log', 'h5', 'h8'])
    expect(w.calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=auto', 'as_of=42', 'kept_turns=1', 'reason=tail-trimmed'])
  })

  test('a newest prompt bigger than tail_chars falls back to the engine with reason=tail-too-large (I1)', async ($, on) => {
    const tiny = JSON.stringify({ v: 1, as_of: 42, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 3, tail_chars: 2 } })
    const w = world(on, { 'context --json': { exitCode: 0, stdout: tiny }, 'append rebuild': OK })
    await w.start($)
    const r = await $.session.compact({ trigger: 'auto', messages: msgs })
    expect(w.summarized).toEqual(['auto'])
    expect(r.messages?.map(m => m.text)).toEqual(['old'])
    expect(w.calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=auto', 'reason=tail-too-large'])
  })

  test('a newest turn exactly at tail_chars still rebuilds', async ($, on) => {
    // 'new' and 'new answer' size to their text lengths, 3 and 10: 13 total.
    const exact = JSON.stringify({ v: 1, as_of: 42, markdown: '# Context rebuilt from the event log', settings: { keep_turns: 3, tail_chars: 13 } })
    const w = world(on, { 'context --json': { exitCode: 0, stdout: exact }, 'append rebuild': OK })
    await w.start($)
    const r = await $.session.compact({ trigger: 'manual', messages: msgs })
    expect(w.summarized).toEqual([])
    expect(r.messages?.map(m => m.text)).toEqual(['# Context rebuilt from the event log', 'new', 'new answer'])
    expect(w.calls).toContainEqual(['eventlog', 'append', 'rebuild', 'trigger=manual', 'as_of=42', 'kept_turns=1'])
  })

  test('every eventlog call runs in the repo root', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: PACKET }, 'append rebuild': OK, 'context check': no('mid-task') }, { tokens: 60_000, percent: 30 })
    await w.start($)
    await $.turn.complete(turn())
    await $.session.compact({ trigger: 'manual', messages: msgs })
    expect(w.calls).toHaveLength(3)
    expect(w.cwds).toEqual(['/repo', '/repo', '/repo'])
    expect(w.git).toEqual([['git', 'rev-parse', '--show-toplevel']])
    expect(w.exists).toEqual(['/repo/.context/events.jsonl'])
  })

  test('a subagent compaction passes through and runs no eventlog process', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: PACKET } })
    await w.start($)
    await $.session.compact({ trigger: 'auto', agentId: 'sub-1', messages: msgs })
    expect(w.summarized).toEqual(['auto'])
    expect(w.calls).toEqual([])
  })
})

describe('turn.complete', () => {
  const PACKET_OK = { 'context --json': { exitCode: 0, stdout: PACKET }, 'append rebuild': OK }

  test('check says rebuild: the mod\'s own compact hook answers, not the summarizer', { plugins: [observer] }, async ($, on) => {
    const w = world(on, { ...PACKET_OK, 'context check': yes('boundary') }, { tokens: 60_000, window: 200_000, percent: 30 })
    await w.start($)
    const r = await $.turn.complete(turn())
    expect(r.text).toBe('ok')
    expect(w.runs('context check')).toHaveLength(1)
    expect(w.summarized).toEqual([])
    expect(w.runs('context --json')).toHaveLength(1)
    expect(w.runs('append rebuild')).toEqual([['eventlog', 'append', 'rebuild', 'trigger=boundary', 'as_of=42', 'tokens_before=60000', 'kept_turns=1']])
    // What the mod's hook handed up: the packet, then the newest turn whole.
    const seen = w.logs.filter(l => l.startsWith(OBSERVED)).map(l => JSON.parse(l.slice(OBSERVED.length)))
    expect(seen).toEqual([[['# Context rebuilt from the event log', null], ['new', 'h3'], ['new answer', 'h4']]])
    expect(w.logs.filter(l => l.includes('read the transcript'))).toEqual([])
  })

  test('a compaction raised without messages reads the transcript and says so once', async ($, on) => {
    const w = world(on, { ...PACKET_OK, 'context check': yes('boundary') }, { tokens: 60_000, percent: 30 })
    await w.start($)
    await $.turn.complete(turn())
    expect(w.summarized).toEqual([])
    expect(w.runs('append rebuild')).toEqual([['eventlog', 'append', 'rebuild', 'trigger=boundary', 'as_of=42', 'tokens_before=60000', 'kept_turns=1']])
    expect(w.logs.filter(l => l.includes('read the transcript'))).toEqual([
      'eventlog-context: session.compact came without messages; read the transcript with $.session.messages()',
    ])
  })

  test('check says no: nothing compacts', async ($, on) => {
    const w = world(on, { ...PACKET_OK, 'context check': no('mid-task') }, { tokens: 60_000, percent: 30 })
    await w.start($)
    await $.turn.complete(turn())
    expect(w.runs('context check')).toHaveLength(1)
    expect(w.runs('context --json')).toEqual([])
    expect(w.runs('append rebuild')).toEqual([])
    expect(w.summarized).toEqual([])
  })

  test('check times out: nothing compacts, the turn stands', async ($, on) => {
    const w = world(on, { ...PACKET_OK, 'context check': 'throw' }, { tokens: 60_000, percent: 30 })
    await w.start($)
    const r = await $.turn.complete(turn())
    expect(r.text).toBe('ok')
    expect(w.runs('context check')).toHaveLength(1)
    expect(w.runs('context --json')).toEqual([])
    expect(w.logs.some(l => l.includes('check failed'))).toBe(true)
  })

  test('check prints nothing: nothing compacts, the turn stands', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 0, stdout: PACKET }, 'context check': OK }, { tokens: 60_000, percent: 30 })
    await w.start($)
    const r = await $.turn.complete(turn())
    expect(r.text).toBe('ok')
    expect(w.runs('context check')).toHaveLength(1)
    expect(w.runs('context --json')).toEqual([])
  })

  test('a missing reading skips the check', async ($, on) => {
    const w = world(on, { ...PACKET_OK, 'context check': yes('boundary') }, { window: 200_000, threshold: 180_000 })
    await w.start($)
    await $.turn.complete(turn())
    expect(w.calls).toEqual([])
  })

  test('fill is tokens over the auto-compact threshold when the breakdown has one', async ($, on) => {
    const w = world(on, { 'context check': no('mid-task') }, { tokens: 100_000, window: 1_000_000, percent: 10, threshold: 200_000 })
    await w.start($)
    await $.turn.complete(turn())
    expect(w.runs('context check')).toEqual([['eventlog', 'context', 'check', '--percent', '50', '--growth', '0']])
  })

  test('fill falls back to context.percent without a threshold', async ($, on) => {
    const w = world(on, { 'context check': no('mid-task') }, { tokens: 100_000, window: 1_000_000, percent: 10 })
    await w.start($)
    await $.turn.complete(turn())
    expect(w.runs('context check')).toEqual([['eventlog', 'context', 'check', '--percent', '10', '--growth', '0']])
  })

  test('growth is the average per-turn increase, passed as --growth', async ($, on) => {
    const usage: Usage = { window: 1_000_000, threshold: 200_000 }
    const w = world(on, { 'context check': no('mid-task') }, usage)
    await w.start($)
    for (const tokens of [20_000, 40_000, 60_000]) {
      usage.tokens = tokens
      await $.turn.complete(turn())
    }
    expect(w.runs('context check').map(c => c.slice(4))).toEqual([
      ['10', '--growth', '0'],
      ['20', '--growth', '10'],
      ['30', '--growth', '10'],
    ])
  })

  test('growth history clears after a compaction', async ($, on) => {
    const usage: Usage = { window: 1_000_000, threshold: 200_000 }
    const w = world(on, { ...PACKET_OK, 'context check': no('mid-task') }, usage)
    await w.start($)
    for (const tokens of [20_000, 60_000]) {
      usage.tokens = tokens
      await $.turn.complete(turn())
    }
    await $.session.compact({ trigger: 'manual', messages: msgs })
    for (const tokens of [30_000, 40_000]) {
      usage.tokens = tokens
      await $.turn.complete(turn())
    }
    expect(w.runs('context check').map(c => c[6])).toEqual(['0', '20', '0', '5'])
  })

  test('subagent turns and non-answers run nothing', async ($, on) => {
    const w = world(on, { ...PACKET_OK, 'context check': yes('boundary') }, { tokens: 60_000, percent: 30 })
    await w.start($)
    await $.turn.complete(turn({ agentId: 'sub-1' }))
    await $.turn.complete(turn({ reason: 'aborted', isAborted: true }))
    await $.turn.complete(turn({ reason: 'error' }))
    expect(w.calls).toEqual([])
  })

  test('a skipped compaction is logged', { plugins: [vetoer] }, async ($, on) => {
    const w = world(on, { ...PACKET_OK, 'context check': yes('boundary') }, { tokens: 60_000, percent: 30 })
    await w.start($)
    const r = await $.turn.complete(turn())
    expect(r.text).toBe('ok')
    expect(w.logs.some(l => l.includes('rebuild skipped') && l.includes('vetoed by a test plugin'))).toBe(true)
  })

  test('a rejected compaction is caught and logged; the turn stands', async ($, on) => {
    const w = world(on, { 'context --json': { exitCode: 1, stdout: '' }, 'append rebuild': OK, 'context check': yes('backstop') }, { tokens: 150_000, percent: 75 }, { summarizerThrows: true })
    await w.start($)
    const r = await $.turn.complete(turn())
    expect(r.text).toBe('ok')
    expect(w.logs.some(l => l.includes('eventlog-context') && l.includes('compact'))).toBe(true)
  })
})

describe('/rebuild', () => {
  // The host refuses $.session.compact inside command.run, so /rebuild queues
  // and the next main-loop turn.complete rebuilds directly.
  const world2 = (on: On) =>
    world(on, { 'context --json': { exitCode: 0, stdout: PACKET }, 'append rebuild': OK, 'context check': no('mid-task') }, { tokens: 60_000, percent: 30 })

  test('registers the command; queues without compacting; the next turn rebuilds with trigger=command', async ($, on) => {
    const w = world2(on)
    await w.start($)
    expect(w.commands).toContain('rebuild')
    const r = await $.command.run(rebuildCmd())
    expect(r.text).toContain('queued')
    expect(w.calls).toEqual([])
    await $.turn.complete(turn())
    expect(w.runs('context check')).toEqual([])
    expect(w.runs('append rebuild').map(c => c[3])).toEqual(['trigger=command'])
    expect(w.summarized).toEqual([])
    await $.turn.complete(turn())
    expect(w.runs('append rebuild')).toHaveLength(1)
    expect(w.runs('context check')).toHaveLength(1)
  })

  test('the queue waits past subagent and aborted turns', async ($, on) => {
    const w = world2(on)
    await w.start($)
    await $.command.run(rebuildCmd())
    await $.turn.complete(turn({ agentId: 'sub-1' }))
    await $.turn.complete(turn({ reason: 'aborted', isAborted: true }))
    expect(w.calls).toEqual([])
    await $.turn.complete(turn())
    expect(w.runs('append rebuild').map(c => c[3])).toEqual(['trigger=command'])
  })

  test('a /rebuild queued before a new session does not fire after it', async ($, on) => {
    const w = world2(on)
    await w.start($)
    await $.command.run(rebuildCmd())
    await w.start($)
    await $.turn.complete(turn())
    expect(w.runs('append rebuild')).toEqual([])
    expect(w.runs('context check')).toHaveLength(1)
  })

  test('a compaction before the next turn clears the queue', async ($, on) => {
    const w = world2(on)
    await w.start($)
    await $.command.run(rebuildCmd())
    await $.session.compact({ trigger: 'manual', messages: msgs })
    await $.turn.complete(turn())
    expect(w.runs('append rebuild').map(c => c[3])).toEqual(['trigger=manual'])
    expect(w.runs('context check')).toHaveLength(1)
  })
})
