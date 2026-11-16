// arx's claude-code main sidecar, b8d59164
import { query, createSdkMcpServer, tool, SYSTEM_PROMPT_DYNAMIC_BOUNDARY, USAGE_LIMIT_ERROR_PREFIXES } from '@anthropic-ai/claude-agent-sdk';
import readline from 'node:readline';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { z } from 'zod';

function writeEvent(ev) {
  process.stdout.write(JSON.stringify(ev) + '\n');
}

let nextStoreId = 1;
const pendingStores = new Map();

function callStore(verb, args) {
  const id = nextStoreId++;
  writeEvent({ ev: 'store', id, verb, args });
  return new Promise((resolve) => {
    pendingStores.set(id, resolve);
  });
}

function storeToolResult(result) {
  return { content: [{ type: 'text', text: JSON.stringify(result) }] };
}

let nextHandId = 1;
const pendingHands = new Map();

function callHand(verb, args) {
  const id = nextHandId++;
  writeEvent({ ev: 'hand', id, verb, args });
  return new Promise((resolve) => {
    pendingHands.set(id, resolve);
  });
}

function handToolResult(result) {
  if (typeof result.text === 'string') {
    return { content: [{ type: 'text', text: result.text }] };
  }
  return { content: [{ type: 'text', text: JSON.stringify(result) }] };
}

let nextFileId = 1;
const pendingFiles = new Map();

function callFile(tool, args) {
  const id = nextFileId++;
  writeEvent({ ev: 'file', id, tool, args });
  return new Promise((resolve) => {
    pendingFiles.set(id, resolve);
  });
}

function fileToolResult(result) {
  return { content: [{ type: 'text', text: result.text ?? result.error }] };
}

process.on('warning', (warning) => {
  if (warning.code === 'CLAUDE_SDK_CAN_USE_TOOL_SHADOWED') {
    return;
  }
  process.stderr.write(`${warning.name}: ${warning.message}\n`);
});

export const arxTools = [
  tool('arx_ping', 'Probe that answers pong. Confirms the harness is listening; no other use.', {}, async () => ({
    content: [{ type: 'text', text: 'pong' }],
  })),
  tool(
    'save',
    "Put a claim candidate into this thread's quarantine and get back the claims already in force beside it. kind is fact or decision; why says what was rejected or what the code does not show. Nothing reaches the store before drain.",
    {
      scope: z.string(),
      kind: z.enum(['fact', 'decision']),
      question: z.string(),
      answer: z.string(),
      why: z.string(),
    },
    async (args) => storeToolResult(await callStore('save', args)),
  ),
  tool(
    'recall',
    "Search a scope's claims in force for a query and return the matches. The result lands here as a tool result and changes nothing above it.",
    {
      scope: z.string(),
      query: z.string(),
      limit: z.number().int().optional(),
    },
    async (args) => storeToolResult(await callStore('recall', args)),
  ),
  tool(
    'decisions_query',
    "Search a scope's decisions in force for a query and return the matches with their ids. Use it for what is normal here before deciding anew.",
    {
      scope: z.string(),
      query: z.string(),
      limit: z.number().int().optional(),
    },
    async (args) => storeToolResult(await callStore('decisions_query', args)),
  ),
  tool(
    'decision',
    'Fetch one claim by id from a scope, in full.',
    {
      scope: z.string(),
      id: z.string(),
    },
    async (args) => storeToolResult(await callStore('decision', args)),
  ),
  tool(
    'drain',
    "Drain this thread's quarantine when the work is done. Without proposal: dispatch the drain hand over the quarantine; its proposal arrives as a [hand … drain proposal] line. With proposal set to that hand's id: apply it as one commit; overrides replaces the action for any candidate you read differently.",
    {
      proposal: z.string().optional(),
      note: z.string().optional(),
      overrides: z
        .array(
          z.object({
            id: z.string(),
            action: z.enum(['promote', 'drop']),
            supersedes: z.string().optional(),
            reason: z.string(),
          }),
        )
        .optional(),
    },
    async (args) => storeToolResult(await callStore('drain', args)),
  ),
  tool(
    'dispatch',
    "Start a hand from a named agent definition with a self-contained brief and return its id at once; nothing blocks. The hand sees the brief and nothing else: not this window, not the store, not the state object. Its verdict, stop or exit arrives later as a [hand …] line with a trace handle. body, if given, must match the definition's.",
    { agent: z.string(), body: z.string().optional(), brief: z.string() },
    async (args) => handToolResult(await callHand('dispatch', args)),
  ),
  tool(
    'tap',
    "Read a hand's state, its brief, and the question it is stopped on, if any.",
    { hand: z.string() },
    async (args) => handToolResult(await callHand('tap', args)),
  ),
  tool(
    'steer',
    'Answer a stopped hand, or push a message into a running native hand. A claude-code or codex hand cannot take a message mid-turn; kill it and dispatch again.',
    { hand: z.string(), message: z.string() },
    async (args) => handToolResult(await callHand('steer', args)),
  ),
  tool(
    'kill',
    'Tear the hand down. Its exit and trace handle arrive as a [hand …] line.',
    { hand: z.string() },
    async (args) => handToolResult(await callHand('kill', args)),
  ),
  tool(
    'read_file',
    'Read one UTF-8 text file under the workspace. path is relative to the workspace, or absolute inside it; offset is a 1-based line, limit a line count. A result over the admission threshold arrives as its head plus a handle with byte and line counts; read the handle with offset and limit for the part you need.',
    { path: z.string(), offset: z.number().int().min(1).optional(), limit: z.number().int().min(1).optional() },
    async (args) => fileToolResult(await callFile('read_file', args)),
  ),
  tool(
    'edit_file',
    "Replace one exact occurrence of old with new in one existing file under the workspace. Fails when old is absent or matches more than once, and refuses arx's own state (store/, threads/, pid.txt). For a fix you can see whole; anything wider is a hand's brief.",
    { path: z.string(), old: z.string(), new: z.string() },
    async (args) => fileToolResult(await callFile('edit_file', args)),
  ),
  tool(
    'list_files',
    'List file and directory paths under the workspace. path is a directory relative to the workspace and defaults to the workspace root; depth is how many levels to descend, default 2, capped at 8. Directories end in a slash. Dotfiles, target/, node_modules/ and arx\'s own state (store/, threads/, pid.txt) are omitted, and a long listing arrives as a handle like any other oversized result. For seeing what is there; reading it is read_file, searching it is a hand.',
    { path: z.string().optional(), depth: z.number().int().min(1).max(8).optional() },
    async (args) => fileToolResult(await callFile('list_files', args)),
  ),
];

const arxServer = createSdkMcpServer({
  name: 'arx',
  tools: arxTools,
});

export function promptContent(text, images) {
  if (!images || !images.length) {
    return text;
  }
  const blocks = [{ type: 'text', text }];
  for (const image of images) {
    const data = readFileSync(image.path).toString('base64');
    blocks.push({ type: 'image', source: { type: 'base64', media_type: image.media_type, data } });
  }
  return blocks;
}

function createPromptQueue() {
  const queue = [];
  let resolveNext = null;
  let ended = false;
  return {
    push(text, images) {
      const message = {
        type: 'user',
        message: { role: 'user', content: promptContent(text, images) },
        parent_tool_use_id: null,
      };
      if (resolveNext) {
        const resolve = resolveNext;
        resolveNext = null;
        resolve({ value: message, done: false });
      } else {
        queue.push(message);
      }
    },
    end() {
      ended = true;
      if (resolveNext) {
        const resolve = resolveNext;
        resolveNext = null;
        resolve({ value: undefined, done: true });
      }
    },
    [Symbol.asyncIterator]() {
      return {
        next() {
          if (queue.length > 0) {
            return Promise.resolve({ value: queue.shift(), done: false });
          }
          if (ended) {
            return Promise.resolve({ value: undefined, done: true });
          }
          return new Promise((resolve) => {
            resolveNext = resolve;
          });
        },
      };
    },
  };
}

function modelUsageTotal(usage) {
  return (
    (usage?.inputTokens ?? 0) +
    (usage?.outputTokens ?? 0) +
    (usage?.cacheReadInputTokens ?? 0) +
    (usage?.cacheCreationInputTokens ?? 0)
  );
}

function modelsGrown(prev, current) {
  const grown = [];
  for (const [model, usage] of Object.entries(current ?? {})) {
    if (modelUsageTotal(usage) > modelUsageTotal(prev[model])) {
      grown.push(model);
    }
  }
  return grown;
}

function isUsageLimitError(message) {
  return USAGE_LIMIT_ERROR_PREFIXES.some((prefix) => message.startsWith(prefix));
}

export async function drive(q, { emit, close, consumeInterrupt = () => false }) {
  let turnText = '';
  let sawAssistantText = false;
  let prevModelUsage = {};
  for await (const message of q) {
    if (message.type === 'stream_event') {
      const event = message.event;
      if (event.type === 'content_block_delta' && event.delta.type === 'text_delta') {
        emit({ ev: 'delta', text: event.delta.text });
      }
    } else if (message.type === 'assistant' && message.parent_tool_use_id === null) {
      for (const block of message.message.content ?? []) {
        if (block.type === 'text') {
          turnText += block.text;
          sawAssistantText = true;
        }
      }
    } else if (message.type === 'system' && message.subtype === 'model_refusal_fallback') {
      if (message.scope !== 'local') {
        emit({ ev: 'model_fallback', from: message.original_model, to: message.fallback_model });
      }
    } else if (message.type === 'result') {
      const interrupted = consumeInterrupt();
      if (message.is_error && !interrupted) {
        const detail = Array.isArray(message.errors) && message.errors.length
          ? message.errors.join('; ')
          : (message.result || message.subtype);
        emit({ ev: isUsageLimitError(detail) ? 'usage_limit' : 'fatal', message: detail });
        close();
      } else {
        if (process.env.ARX_SIDECAR_TRACE) {
          process.stderr.write(`[trace] modelUsage keys: ${JSON.stringify(Object.keys(message.modelUsage ?? {}))}\n`);
        }
        const text = sawAssistantText ? turnText : (message.result ?? '');
        const models = modelsGrown(prevModelUsage, message.modelUsage);
        emit({ ev: 'turn', text });
        if (message.usage) {
          emit({ ev: 'usage', usage: message.usage, models });
        }
        const turnEnd = { ev: 'turn_end' };
        if (interrupted) {
          turnEnd.stopped = true;
        }
        emit(turnEnd);
        prevModelUsage = message.modelUsage ?? {};
      }
      turnText = '';
      sawAssistantText = false;
    }
  }
}

let activeQuery = null;
let interruptRequested = false;

async function postModelSwitchHook(input) {
  writeEvent({ ev: 'model_changed', model: input.to_model, source: input.source });
  return {};
}

function fail(err) {
  writeEvent({ ev: 'fatal', message: String(err && err.message ? err.message : err) });
  activeQuery?.close();
  process.exit(1);
}

function stop() {
  activeQuery?.close();
  process.exit(0);
}

async function main() {
  const promptQueue = createPromptQueue();
  const rl = readline.createInterface({ input: process.stdin, terminal: false });

  rl.on('line', (line) => {
    if (!line.trim()) {
      return;
    }
    const op = JSON.parse(line);
    if (op.op === 'init') {
      const q = query({
        prompt: promptQueue,
        options: {
          model: op.model,
          systemPrompt: [op.systemPrompt.static, SYSTEM_PROMPT_DYNAMIC_BOUNDARY, op.systemPrompt.dynamic],
          settingSources: [],
          tools: [],
          includePartialMessages: true,
          cwd: op.cwd,
          mcpServers: { arx: arxServer },
          allowedTools: arxTools.map((t) => 'mcp__arx__' + t.name),
          hooks: { PostModelSwitch: [{ hooks: [postModelSwitchHook] }] },
        },
      });
      activeQuery = q;
      drive(q, {
        emit: writeEvent,
        close: () => activeQuery?.close(),
        consumeInterrupt: () => {
          const was = interruptRequested;
          interruptRequested = false;
          return was;
        },
      }).catch(fail);
      q.initializationResult()
        .then(() => writeEvent({ ev: 'ready' }))
        .catch(fail);
    } else if (op.op === 'prompt') {
      promptQueue.push(op.text, op.images);
    } else if (op.op === 'set_model') {
      if (!activeQuery) {
        writeEvent({ ev: 'model_set', model: op.model, error: 'no active session' });
      } else {
        activeQuery
          .setModel(op.model)
          .then(() => writeEvent({ ev: 'model_set', model: op.model }))
          .catch((err) => writeEvent({ ev: 'model_set', model: op.model, error: String(err && err.message ? err.message : err) }));
      }
    } else if (op.op === 'store_result') {
      const resolve = pendingStores.get(op.id);
      if (resolve) {
        pendingStores.delete(op.id);
        resolve(op.result);
      }
    } else if (op.op === 'hand_result') {
      const resolve = pendingHands.get(op.id);
      if (resolve) {
        pendingHands.delete(op.id);
        resolve(op.result);
      }
    } else if (op.op === 'file_result') {
      const resolve = pendingFiles.get(op.id);
      if (resolve) {
        pendingFiles.delete(op.id);
        resolve(op.result);
      }
    } else if (op.op === 'interrupt') {
      if (activeQuery) {
        interruptRequested = true;
        activeQuery.interrupt().catch(() => {});
      }
    } else if (op.op === 'shutdown') {
      promptQueue.end();
      stop();
    }
  });

  rl.on('close', () => {
    promptQueue.end();
    stop();
  });
}

if (process.argv[1] && process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch(fail);
}
