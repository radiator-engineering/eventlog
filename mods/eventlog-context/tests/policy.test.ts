import type { SessionMessage } from 'claude-code'
import { describe, expect, test } from 'claude-code/testing'

import {
  LOG_FILE,
  PACKET_HEADING,
  REBUILD_PREFIX,
  checkArgs,
  fillOf,
  growthOf,
  controllerPaneOf,
  inertReason,
  installRoot,
  isPacket,
  isTurnStart,
  markPacket,
  parsePacket,
  parseVerdict,
  pushIncrease,
  readPacket,
  rebuildArgs,
  selectTail,
  newestTurnSize,
  newestTurnTrimmed,
  trimTurn,
  tailTooLarge,
  triggerOf,
} from '../hooks/policy.ts'

const user = (text: string): SessionMessage => ({ role: 'user', text, toolUses: [], handle: `h-${text}` })
const toolResult = (text: string): SessionMessage => ({ role: 'user', text, toolUses: [], toolResults: [{ tool_use_id: 't', text, isError: false } as never], handle: `h-${text}` })
const asst = (text: string): SessionMessage => ({ role: 'assistant', text, toolUses: [], handle: `h-${text}` })

describe('policy: tail', () => {
  test('isTurnStart is a user message without tool results', () => {
    expect(isTurnStart(user('u'))).toBe(true)
    expect(isTurnStart(toolResult('r'))).toBe(false)
    expect(isTurnStart(asst('a'))).toBe(false)
    expect(isTurnStart({ role: 'user', text: 'u', toolUses: [], toolResults: [] })).toBe(true)
  })

  test('selectTail keeps the last N turns, starting at a real user message', () => {
    const msgs = [user('u1'), asst('a1'), user('u2'), asst('a2'), toolResult('r2'), asst('a2b'), user('u3'), asst('a3')]
    const tail = selectTail(msgs, 2, 1_000_000)
    expect(tail.map(m => m.text)).toEqual(['u2', 'a2', 'r2', 'a2b', 'u3', 'a3'])
  })

  test('selectTail never starts at a tool result', () => {
    const msgs = [user('u1'), asst('a1'), toolResult('r1'), asst('a1b')]
    expect(selectTail(msgs, 5, 1_000_000)[0].text).toBe('u1')
  })

  test('selectTail drops messages before the first turn start', () => {
    const msgs = [toolResult('r0'), asst('a0'), user('u1'), asst('a1')]
    expect(selectTail(msgs, 5, 1_000_000).map(m => m.text)).toEqual(['u1', 'a1'])
  })

  test('selectTail drops oldest turns to fit tail_chars but keeps the newest', () => {
    const big = 'x'.repeat(500)
    const msgs = [user(big), asst(big), user('u2'), asst(big.slice(0, 300))]
    const tail = selectTail(msgs, 3, 600)
    expect(tail.map(m => m.text.slice(0, 2))).toEqual(['u2', 'xx'])
  })

  test('selectTail keeps handles', () => {
    expect(selectTail([user('u1'), asst('a1')], 1, 1000)[1].handle).toBe('h-a1')
  })

  test('selectTail with no turns or keepTurns 0 is empty', () => {
    expect(selectTail([asst('a')], 3, 1000)).toEqual([])
    expect(selectTail([user('u'), asst('a')], 0, 1000)).toEqual([])
  })

  test('an oversized newest turn keeps only its prompt', () => {
    const big = 'x'.repeat(500)
    const all = [user('u1'), asst('a1'), user('prompt'), asst(big)]
    expect(newestTurnTrimmed(all, 200)).toBe(true)
    expect(selectTail(all, 3, 200).map(m => m.text)).toEqual(['prompt'])
    expect(newestTurnTrimmed(all, 1000)).toBe(false)
    expect(newestTurnTrimmed([asst('a')], 1)).toBe(false)
  })

  test('trimTurn keeps prompt and final answer when both fit, else the prompt', () => {
    const turn = [user('go'), asst('done')]
    expect(trimTurn(turn, 100).map(m => m.text)).toEqual(['go', 'done'])
    expect(trimTurn(turn, 3).map(m => m.text)).toEqual(['go'])
    expect(trimTurn([user('go')], 100).map(m => m.text)).toEqual(['go'])
    expect(trimTurn([user('go'), asst('')], 100).map(m => m.text)).toEqual(['go'])
  })

  test('size counts a tool output once, as the model reads it', () => {
    const out = 'x'.repeat(1000)
    const all: SessionMessage[] = [
      user('p'),
      { role: 'assistant', text: '', toolUses: [{ tool_use_id: 't', tool: 'Read', input: {}, text: out, result: { content: out } }] },
      { role: 'user', text: '', toolUses: [], toolResults: [{ tool_use_id: 't', text: out, isError: false, result: { content: out } }] },
    ]
    expect(newestTurnSize(all)).toBe(1 + 4 + 2 + 1000)
  })

  test('tailTooLarge is true only when the newest prompt alone cannot fit', () => {
    const big = 'x'.repeat(500)
    expect(tailTooLarge([user('u1'), asst('a1'), user(big), asst(big)], 600)).toBe(false)
    expect(tailTooLarge([user('u1'), asst('a1'), user(big)], 100)).toBe(true)
    expect(tailTooLarge([asst('a')], 1)).toBe(false)
  })
})

describe('policy: earlier packets', () => {
  const packet = (body: string): SessionMessage => ({ role: 'user', text: `${PACKET_HEADING}\n\n${body}`, toolUses: [], handle: 'h-packet' })

  test('isPacket knows a packet by its first line', () => {
    expect(isPacket(packet('x'))).toBe(true)
    expect(isPacket({ role: 'user', text: PACKET_HEADING, toolUses: [] })).toBe(true)
    expect(isPacket(user(`${PACKET_HEADING} and more`))).toBe(false)
    expect(isPacket(user(`intro\n${PACKET_HEADING}`))).toBe(false)
    expect(isPacket({ role: 'assistant', text: PACKET_HEADING, toolUses: [] })).toBe(false)
  })

  test('a packet is not a turn start', () => {
    expect(isTurnStart(packet('x'))).toBe(false)
  })

  test('selectTail drops an earlier packet', () => {
    const tail = selectTail([packet('old'), user('u1'), asst('a1')], 3, 1_000_000)
    expect(tail.map(m => m.text)).toEqual(['u1', 'a1'])
  })

  test('selectTail drops a packet that sits inside a kept turn', () => {
    const tail = selectTail([user('u1'), packet('old'), asst('a1')], 3, 1_000_000)
    expect(tail.map(m => m.text)).toEqual(['u1', 'a1'])
  })

  test('markPacket leads with the heading exactly once', () => {
    expect(markPacket(`${PACKET_HEADING}\n\nbody`)).toBe(`${PACKET_HEADING}\n\nbody`)
    expect(markPacket('# Something else\n\nbody')).toBe(`${PACKET_HEADING}\n\n# Something else\n\nbody`)
    expect(isPacket({ role: 'user', text: markPacket('anything'), toolUses: [] })).toBe(true)
  })
})

describe('policy: which sessions', () => {
  test('installRoot strips the skills folder', () => {
    expect(installRoot('/repo/.claude/skills/eventlog-context')).toBe('/repo')
    expect(installRoot('/repo/.claude/skills/eventlog-context/')).toBe('/repo')
    expect(installRoot('/a b/.claude/skills/other-name')).toBe('/a b')
    expect(installRoot('/repo/mods/eventlog-context')).toBeNull()
    expect(installRoot('/.claude/skills/eventlog-context')).toBeNull()
  })

  test('LOG_FILE is the coordination log under the root', () => {
    expect(LOG_FILE).toBe('.context/events.jsonl')
  })

  test('inertReason names the first rule that holds, or null', () => {
    const ok = { isInteractive: true, writer: undefined, setting: undefined, hasLog: true }
    expect(inertReason(ok)).toBeNull()
    expect(inertReason({ ...ok, writer: 'controller' })).toBeNull()
    expect(inertReason({ ...ok, writer: '' })).toBeNull()
    expect(inertReason({ ...ok, setting: 'on' })).toBeNull()
    expect(inertReason({ ...ok, isInteractive: false })).toBe('headless')
    expect(inertReason({ ...ok, writer: 'worker-1' })).toBe('EVENTLOG_AS=worker-1')
    expect(inertReason({ ...ok, worker: 'doc-worker' })).toBe('LOG_DRIVEN_WORKER=doc-worker')
    expect(inertReason({ ...ok, pane: 'w9:p1', controllerPane: 'w1:p1' })).toBe("pane w9:p1 is not the controller's pane")
    expect(inertReason({ ...ok, pane: 'w1:p1', controllerPane: 'w1:p1' })).toBeNull()
    expect(inertReason({ ...ok, pane: 'w9:p1' })).toBeNull()
    expect(inertReason({ ...ok, controllerPane: 'w1:p1' })).toBeNull()
    expect(inertReason({ ...ok, setting: 'off' })).toBe('EVENTLOG_CONTEXT=off')
    expect(inertReason({ ...ok, setting: 'OFF' })).toBe('EVENTLOG_CONTEXT=off')
    expect(inertReason({ ...ok, hasLog: false })).toBe('no log')
  })
})

describe('policy: parsing', () => {
  test('parsePacket rejects failure, bad JSON, and empty markdown', () => {
    expect(parsePacket({ exitCode: 1, stdout: '{}' })).toBeNull()
    expect(parsePacket({ exitCode: 0, stdout: 'nope' })).toBeNull()
    expect(parsePacket({ exitCode: 0, stdout: '' })).toBeNull()
    expect(parsePacket({ exitCode: 0, stdout: JSON.stringify({ v: 1, as_of: 1, markdown: '', settings: { keep_turns: 3, tail_chars: 10 } }) })).toBeNull()
    expect(parsePacket({ exitCode: 0, stdout: JSON.stringify({ v: 1, as_of: 1, markdown: '# x', settings: { keep_turns: '3', tail_chars: 10 } }) })).toBeNull()
    const ok = parsePacket({ exitCode: 0, stdout: JSON.stringify({ v: 1, as_of: 7, markdown: '# x', settings: { keep_turns: 3, tail_chars: 10 } }) })
    expect(ok?.as_of).toBe(7)
    expect(ok?.settings.keep_turns).toBe(3)
  })

  test('parsePacket rejects as_of <= 0 and a version other than 1', () => {
    const packet = (over: object) => ({ exitCode: 0, stdout: JSON.stringify({ v: 1, as_of: 7, markdown: '# x', settings: { keep_turns: 3, tail_chars: 10 }, ...over }) })
    expect(parsePacket(packet({ as_of: 0 }))).toBeNull()
    expect(parsePacket(packet({ as_of: -1 }))).toBeNull()
    expect(parsePacket(packet({ v: 2 }))).toBeNull()
    expect(parsePacket(packet({ v: undefined }))).toBeNull()
  })

  test('readPacket tells an engine failure from a refused packet, and a version mismatch from an empty log', () => {
    const packet = (over: object) => ({ exitCode: 0, stdout: JSON.stringify({ v: 1, as_of: 7, markdown: '# x', settings: { keep_turns: 3, tail_chars: 10 }, ...over }) })
    expect(readPacket(packet({}))).toEqual({ packet: { as_of: 7, markdown: '# x', settings: { keep_turns: 3, tail_chars: 10 } } })
    expect(readPacket(packet({ as_of: 0 }))).toEqual({ refused: 'empty', detail: 'no events (as_of 0)' })
    expect(readPacket(packet({ v: 2 }))).toEqual({ refused: 'version', detail: 'packet version 2' })
    expect(readPacket({ exitCode: 1, stdout: '' })).toEqual({ error: 'eventlog context failed' })
    expect(readPacket(packet({ markdown: ' ' }))).toEqual({ error: 'eventlog context failed' })
  })

  test('parseVerdict reads the check output', () => {
    expect(parseVerdict({ exitCode: 0, stdout: '{"rebuild":true,"reason":"boundary"}\n' })).toEqual({ rebuild: true, reason: 'boundary' })
    expect(parseVerdict({ exitCode: 2, stdout: '' })).toBeNull()
    expect(parseVerdict({ exitCode: 0, stdout: '' })).toBeNull()
    expect(parseVerdict({ exitCode: 0, stdout: '{"rebuild":"yes","reason":"x"}' })).toBeNull()
  })
})

describe('policy: triggers and argv', () => {
  test('triggerOf maps engine triggers and our own reasons', () => {
    expect(triggerOf('auto')).toBe('auto')
    expect(triggerOf('manual')).toBe('manual')
    expect(triggerOf('plugin', `${REBUILD_PREFIX}boundary`)).toBe('boundary')
    expect(triggerOf('plugin', `${REBUILD_PREFIX}command`)).toBe('command')
    expect(triggerOf('plugin', 'something else')).toBe('plugin')
    expect(triggerOf('plugin')).toBe('plugin')
    expect(triggerOf('manual', `${REBUILD_PREFIX}boundary`)).toBe('manual')
  })

  test('triggerOf reads a plugin compaction that arrives without a trigger', () => {
    expect(triggerOf(undefined, `${REBUILD_PREFIX}command`)).toBe('command')
    expect(triggerOf(undefined)).toBe('plugin')
  })

  test('rebuildArgs builds the append argv', () => {
    expect(rebuildArgs('auto', { as_of: 7, kept_turns: 3 })).toEqual(['eventlog', 'append', 'rebuild', 'trigger=auto', 'as_of=7', 'kept_turns=3'])
    expect(rebuildArgs('plugin', { reason: 'engine-fallback' })).toEqual(['eventlog', 'append', 'rebuild', 'trigger=plugin', 'reason=engine-fallback'])
  })

  test('checkArgs passes percent and growth', () => {
    expect(checkArgs(42, 3)).toEqual(['eventlog', 'context', 'check', '--percent', '42', '--growth', '3'])
  })
})

describe('policy: fill and growth', () => {
  test('fillOf prefers tokens over the auto-compact threshold', () => {
    expect(fillOf({ tokens: 50_000, window: 1_000_000, percent: 5, threshold: 200_000 })).toEqual({ percent: 25, limit: 200_000 })
  })

  test('fillOf falls back to context.percent against the window', () => {
    expect(fillOf({ tokens: 50_000, window: 200_000, percent: 25 })).toEqual({ percent: 25, limit: 200_000 })
    expect(fillOf({ window: 200_000, percent: 31, threshold: 180_000 })).toEqual({ percent: 31, limit: 200_000 })
  })

  test('fillOf returns null with no reading, never 0', () => {
    expect(fillOf({ window: 200_000 })).toBeNull()
    expect(fillOf({ window: 200_000, threshold: 180_000 })).toBeNull()
    expect(fillOf({})).toBeNull()
  })

  test('fillOf clamps and rounds', () => {
    expect(fillOf({ tokens: 250_000, threshold: 200_000, window: 1_000_000 })).toEqual({ percent: 100, limit: 200_000 })
    expect(fillOf({ tokens: 1_001, threshold: 2_000, window: 1_000_000 })).toEqual({ percent: 50, limit: 2_000 })
    expect(fillOf({ percent: -3, window: 10 })).toEqual({ percent: 0, limit: 10 })
    expect(fillOf({ percent: 44.6, window: 10 })).toEqual({ percent: 45, limit: 10 })
  })

  test('pushIncrease records positive increases and keeps the last 5', () => {
    let h: number[] = []
    h = pushIncrease(h, undefined, 1000)
    expect(h).toEqual([])
    h = pushIncrease(h, 1000, 1500)
    expect(h).toEqual([500])
    h = pushIncrease(h, 1500, 1200)
    expect(h).toEqual([500])
    h = pushIncrease(h, 1200, undefined)
    expect(h).toEqual([500])
    for (let i = 1; i <= 6; i++) h = pushIncrease(h, 0, i * 100)
    expect(h).toEqual([200, 300, 400, 500, 600])
  })

  test('pushIncrease does not mutate its input', () => {
    const h = [1, 2]
    pushIncrease(h, 0, 5)
    expect(h).toEqual([1, 2])
  })

  test('growthOf is the average increase as a percent of the limit', () => {
    expect(growthOf([], 200_000)).toBe(0)
    expect(growthOf([10_000, 20_000], 200_000)).toBe(8)
    expect(growthOf([10_000], 0)).toBe(0)
    expect(growthOf([500_000], 200_000)).toBe(100)
  })
})

describe('policy: controller pane', () => {
  test('controllerPaneOf reads .controller.pane and nothing else', () => {
    expect(controllerPaneOf('{"controller":{"pane":"w1:p1"}}')).toBe('w1:p1')
    expect(controllerPaneOf(undefined)).toBeUndefined()
    expect(controllerPaneOf('not json')).toBeUndefined()
    expect(controllerPaneOf('{"controller":{}}')).toBeUndefined()
    expect(controllerPaneOf('{"controller":{"pane":""}}')).toBeUndefined()
    expect(controllerPaneOf('{"controller":{"pane":7}}')).toBeUndefined()
  })
})
