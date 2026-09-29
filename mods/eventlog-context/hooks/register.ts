import type { On, SessionMessage } from 'claude-code'

import {
  LAYOUT_FILE,
  LOG_FILE,
  REBUILD_PREFIX,
  checkArgs,
  fillOf,
  growthOf,
  controllerPaneOf,
  inertReason,
  installRoot,
  isTurnStart,
  markPacket,
  parseVerdict,
  pushIncrease,
  readPacket,
  rebuildArgs,
  selectTail,
  newestTurnSize,
  newestTurnTrimmed,
  tailTooLarge,
  triggerOf,
} from './policy.ts'

const TIMEOUT_MS = 5_000
const TAG = 'eventlog-context'

const errText = (err: unknown): string => (err instanceof Error ? err.message : String(err))

export function register(on: On): void {
  // The repo root every eventlog call runs in. Undefined means the mod is
  // inert: it stays so until session.start finds the controller's
  // interactive session in a repo that has a log.
  let root: string | undefined
  // Per-turn token increases of the main loop, and the last reading they
  // are measured from. Cleared by every main-loop compaction.
  let increases: number[] = []
  let lastTokens: number | undefined
  // Set by /rebuild; the next main-loop turn.complete compacts.
  let pendingCommand = false

  const forget = (): void => {
    increases = []
    lastTokens = undefined
    pendingCommand = false
  }

  on('session.start', async ($, e, next) => {
    const started = await next(e)
    root = undefined
    forget()
    try {
      let reason: string | null = 'headless'
      let found: string | null = null
      if (e.isInteractive) {
        const writer = await $.env.get('EVENTLOG_AS')
        const setting = await $.env.get('EVENTLOG_CONTEXT')
        const worker = await $.env.get('LOG_DRIVEN_WORKER')
        const pane = await $.env.get('HERDR_PANE_ID')
        // Installed at <root>/.claude/skills/<name>/; from anywhere else
        // (a --plugin-dir checkout), the git top level of the session.
        found = installRoot($.plugin.root)
        if (found === null) {
          const git = await $.process.run(['git', 'rev-parse', '--show-toplevel'], { cwd: e.cwd, timeoutMs: TIMEOUT_MS })
          found = git.exitCode === 0 && git.stdout.trim() !== '' ? git.stdout.trim() : null
        }
        const hasLog = found !== null && (await $.fs.exists(`${found}/${LOG_FILE}`))
        let controllerPane: string | undefined
        if (pane && found !== null && (await $.fs.exists(`${found}/${LAYOUT_FILE}`))) {
          controllerPane = controllerPaneOf(await $.fs.read(`${found}/${LAYOUT_FILE}`))
        }
        reason = inertReason({ isInteractive: true, writer, setting, hasLog, worker, pane, controllerPane })
      }
      if (reason !== null || found === null) {
        $.ui.log(`${TAG}: inert in this session (${reason ?? 'no log'})`)
        return started
      }
      root = found
      await $.command.register({ name: 'rebuild', description: 'Rebuild the context from the event log when the current turn ends' })
    } catch (err) {
      $.ui.log(`${TAG}: inert in this session (setup failed: ${errText(err)})`)
    }
    return started
  })

  // The host refuses $.session.compact from a command.run hook: it would
  // compact under the turn the hook holds. /rebuild queues the rebuild, and
  // the next main-loop turn.complete runs it directly.
  on('command.run', { command: 'rebuild' }, async ($, e, next) => {
    if (root === undefined) return next(e)
    pendingCommand = true
    return { text: `${TAG}: rebuild queued; it runs when the next turn ends. /compact rebuilds from the log now.` }
  })

  on('turn.complete', async ($, e, next) => {
    const result = await next(e)
    const cwd = root
    if (cwd === undefined || e.agentId !== undefined || e.reason !== 'answer') return result
    let reason: string | undefined
    if (pendingCommand) {
      pendingCommand = false
      reason = 'command'
    } else {
      try {
        const { context } = await $.session.usage({ breakdown: 'summary' })
        const tokens = context.tokens
        increases = pushIncrease(increases, lastTokens, tokens)
        if (tokens !== undefined) lastTokens = tokens
        const fill = fillOf({
          tokens,
          window: context.window,
          percent: context.percent,
          threshold: context.breakdown?.autoCompactThreshold,
        })
        if (!fill) return result
        const run = await $.process.run(checkArgs(fill.percent, growthOf(increases, fill.limit)), { cwd, timeoutMs: TIMEOUT_MS })
        const verdict = parseVerdict(run)
        if (!verdict?.rebuild) return result
        reason = verdict.reason
      } catch (err) {
        $.ui.log(`${TAG}: check failed: ${errText(err)}`)
        return result
      }
    }
    // Direct, never deferred: a deferred call skips this mod's own
    // session.compact hook and runs the engine's summarizer (spike, Q2).
    // Headless sessions reject the call; catch it and carry on.
    try {
      const r = await $.session.compact({ instructions: `${REBUILD_PREFIX}${reason}` })
      if (r.skip) $.ui.log(`${TAG}: rebuild skipped: ${r.skip}`)
    } catch (err) {
      $.ui.log(`${TAG}: compact rejected: ${errText(err)}`)
    }
    return result
  })

  on('session.compact', async ($, e, next) => {
    const cwd = root
    if (cwd === undefined || e.agentId !== undefined) return next(e)
    if (e.trigger === 'precompute') return { skip: `${TAG} rebuilds on demand` }
    forget()
    const trigger = triggerOf(e.trigger, e.instructions)

    const append = async (fields: Record<string, string | number>): Promise<void> => {
      try {
        const r = await $.process.run(rebuildArgs(trigger, fields), { cwd, timeoutMs: TIMEOUT_MS })
        if (r.exitCode !== 0) $.ui.log(`${TAG}: rebuild event not appended: ${r.stderr.trim() || `exit ${r.exitCode}`}`)
      } catch (err) {
        $.ui.log(`${TAG}: rebuild event not appended: ${errText(err)}`)
      }
    }

    let tokensBefore: number | undefined
    try {
      tokensBefore = (await $.session.usage()).context.tokens
    } catch {
      tokensBefore = undefined
    }

    let read: ReturnType<typeof readPacket>
    let messages: readonly SessionMessage[] = []
    try {
      read = readPacket(await $.process.run(['eventlog', 'context', '--json'], { cwd, timeoutMs: TIMEOUT_MS }))
      if ('packet' in read) {
        if (Array.isArray(e.messages)) {
          messages = e.messages
        } else {
          // The engine hands the transcript with handles; an input without
          // it reads the main conversation (rows without handles are rebuilt
          // from their role, text and tool blocks).
          $.ui.log(`${TAG}: session.compact came without messages; read the transcript with $.session.messages()`)
          messages = await $.session.messages()
        }
      }
    } catch (err) {
      read = { error: errText(err) }
    }
    if ('refused' in read) {
      $.ui.log(`${TAG}: packet refused (${read.detail}); the engine compacts`)
      if (read.refused === 'version') {
        // An eventlog upgrade with no reinstall: append so `check` does not
        // find the same boundary again on every turn (M1).
        await append({ reason: 'engine-fallback' })
      }
      // `empty` (no events yet): fall back without appending, so the mod
      // never writes a log that did not exist.
      return next(e)
    }
    if ('error' in read) {
      // Fail open: the engine compacts as it would without this mod. The
      // event goes first so `check` does not find the same boundary again.
      await append({ reason: 'engine-fallback' })
      return next(e)
    }

    const { packet } = read
    if (tailTooLarge(messages, packet.settings.tail_chars)) {
      // The newest turn's prompt alone is bigger than tail_chars: trimming
      // cannot shrink the context enough. Fall back so Claude Code's own
      // summary shrinks it instead (I1).
      await append({ reason: 'tail-too-large' })
      return next(e)
    }
    $.ui.log(`${TAG}: newest turn ${newestTurnSize(messages)} chars, tail_chars ${packet.settings.tail_chars}, ${messages.length} messages`)
    const tail = selectTail(messages, packet.settings.keep_turns, packet.settings.tail_chars)
    const fields: Record<string, string | number> = { as_of: packet.as_of }
    if (tokensBefore !== undefined) fields.tokens_before = tokensBefore
    fields.kept_turns = tail.filter(isTurnStart).length
    if (newestTurnTrimmed(messages, packet.settings.tail_chars)) fields.reason = 'tail-trimmed'
    await append(fields)
    return { messages: [{ role: 'user', text: markPacket(packet.markdown), toolUses: [] }, ...tail] }
  })
}
