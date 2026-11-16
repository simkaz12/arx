import readline from 'node:readline';
import { readFileSync, writeFileSync } from 'node:fs';

function writeEvent(ev) {
  process.stdout.write(JSON.stringify(ev) + '\n');
}

let nextStoreId = 1;
const pendingStores = new Map();
let nextHandId = 1;
const pendingHands = new Map();
let nextFileId = 1;
const pendingFiles = new Map();
let turnsAnswered = 0;
let checkpointAuthoringPromptsAnswered = 0;
let checkpointTruthPromptsAnswered = 0;

function stubDelayMs() {
  const raw = process.env.ARX_STUB_DELAY_MS;
  if (raw === undefined) {
    return 0;
  }
  const value = parseInt(raw, 10);
  return Number.isNaN(value) ? 0 : value;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function delayBeforeReply() {
  const ms = stubDelayMs();
  if (ms > 0) {
    await sleep(ms);
  }
}

function readCwdFile(name) {
  try {
    return readFileSync(name, 'utf8');
  } catch {
    return null;
  }
}

function currentUsage() {
  const raw = process.env.ARX_STUB_TOKENS_PER_TURN;
  if (raw === undefined) {
    return {
      input_tokens: 7,
      cache_creation_input_tokens: 0,
      cache_read_input_tokens: 40,
      output_tokens: 5,
    };
  }
  turnsAnswered += 1;
  const perTurn = parseInt(raw, 10);
  return {
    input_tokens: perTurn * turnsAnswered,
    cache_creation_input_tokens: 0,
    cache_read_input_tokens: 0,
    output_tokens: 5,
  };
}

const rl = readline.createInterface({ input: process.stdin, terminal: false });

rl.on('line', (line) => {
  if (!line.trim()) {
    return;
  }
  const op = JSON.parse(line);
  if (op.op === 'init') {
    writeFileSync('stub-init.json', JSON.stringify({ ...op, compactWindow: process.env.CLAUDE_CODE_AUTO_COMPACT_WINDOW ?? null }));
    writeEvent({ ev: 'ready' });
  } else if (op.op === 'prompt') {
    handlePrompt(op.text);
  } else if (op.op === 'set_model') {
    writeFileSync('stub-set-model.json', JSON.stringify(op));
    writeEvent({ ev: 'model_set', model: op.model });
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
  } else if (op.op === 'shutdown') {
    process.exit(0);
  }
});

rl.on('close', () => {
  process.exit(0);
});


function awaitStoreResult(id) {
  return new Promise((resolve) => {
    pendingStores.set(id, resolve);
  });
}

function awaitHandResult(id) {
  return new Promise((resolve) => {
    pendingHands.set(id, resolve);
  });
}

function awaitFileResult(id) {
  return new Promise((resolve) => {
    pendingFiles.set(id, resolve);
  });
}

async function handlePrompt(text) {
  if (text === 'hang') {
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'stub:' });
    await new Promise(() => {});
    return;
  }
  if (text === 'die') {
    process.exit(1);
  }
  if (text === 'usage-limit') {
    writeEvent({ ev: 'usage_limit', message: "You've reached your weekly limit" });
    return;
  }
  if (text === 'window-exceeded' || (text.startsWith('[hand ') && text.includes('window-exceeded'))) {
    writeEvent({ ev: 'fatal', message: 'prompt is too long: 1000123 tokens > 1000000 maximum' });
    return;
  }
  if (text === 'delta-then-turn-matches') {
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'A' });
    writeEvent({ ev: 'delta', text: 'B' });
    writeEvent({ ev: 'turn', text: 'AB' });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  if (text.startsWith('usage-models:')) {
    const rest = text.slice('usage-models:'.length);
    const models = rest.length ? rest.split(',') : [];
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'stub:' });
    writeEvent({ ev: 'delta', text });
    writeEvent({ ev: 'turn', text: `stub:${text}` });
    writeEvent({ ev: 'usage', usage: currentUsage(), models });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  if (text === 'model-fallback-after-turn-end') {
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'stub:' });
    writeEvent({ ev: 'delta', text });
    writeEvent({ ev: 'turn', text: `stub:${text}` });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    writeEvent({ ev: 'model_fallback', from: 'stub-model', to: 'stub-model-2' });
    return;
  }
  if (text === 'model-fallback-then-fatal') {
    writeEvent({ ev: 'model_fallback', from: 'stub-model', to: 'stub-model-2' });
    writeEvent({ ev: 'fatal', message: 'prompt is too long: 1000123 tokens > 1000000 maximum' });
    return;
  }
  if (text === 'model-fallback') {
    writeEvent({ ev: 'model_fallback', from: 'stub-model', to: 'stub-model-2' });
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'stub:' });
    writeEvent({ ev: 'delta', text });
    writeEvent({ ev: 'turn', text: `stub:${text}` });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  if (text.startsWith('hand:')) {
    const rest = text.slice('hand:'.length);
    const sep = rest.indexOf(':');
    const verb = rest.slice(0, sep);
    const payload = rest.slice(sep + 1);
    let args;
    if (verb === 'dispatch') {
      const agentSep = payload.indexOf(':');
      args = { agent: payload.slice(0, agentSep), brief: payload.slice(agentSep + 1) };
      const at = args.agent.indexOf('@');
      if (at >= 0) {
        args.body = args.agent.slice(at + 1);
        args.agent = args.agent.slice(0, at);
      }
    } else if (verb === 'steer') {
      const idSep = payload.indexOf(':');
      args = { hand: payload.slice(0, idSep), message: payload.slice(idSep + 1) };
    } else {
      args = { hand: payload };
    }
    const id = nextHandId++;
    writeEvent({ ev: 'hand', id, verb, args });
    const result = await awaitHandResult(id);
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'stub:' });
    writeEvent({ ev: 'delta', text });
    writeEvent({ ev: 'turn', text: `stub-hand:${JSON.stringify(result)}` });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  if (text.startsWith('[hand ') && text.includes(' stop]')) {
    const match = text.match(/^\[hand (\S+) stop\]/);
    const targetHand = match ? match[1] : null;
    const steerArgs = { hand: targetHand, message: 'proceed' };
    const id = nextHandId++;
    writeEvent({ ev: 'hand', id, verb: 'steer', args: steerArgs });
    await awaitHandResult(id);
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'stub:' });
    writeEvent({ ev: 'delta', text });
    writeEvent({ ev: 'turn', text: `stub:${text}` });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  if (text.startsWith('[arx] checkpoint ') && text.includes('author the state object')) {
    checkpointAuthoringPromptsAnswered += 1;
    const usageLimitAt = process.env.ARX_STUB_USAGE_LIMIT_AUTHORING_AT;
    if (usageLimitAt !== undefined && checkpointAuthoringPromptsAnswered === parseInt(usageLimitAt, 10)) {
      writeEvent({ ev: 'usage_limit', message: "You've reached your weekly limit" });
      return;
    }
    const fatalAt = process.env.ARX_STUB_FATAL_AUTHORING_AT;
    if (fatalAt !== undefined && checkpointAuthoringPromptsAnswered === parseInt(fatalAt, 10)) {
      writeEvent({ ev: 'fatal', message: 'provider broke during checkpoint authoring' });
      return;
    }
    const rejectAt = process.env.ARX_STUB_REJECT_AUTHORING_AT;
    if (rejectAt !== undefined && checkpointAuthoringPromptsAnswered === parseInt(rejectAt, 10)) {
      writeEvent({ ev: 'fatal', message: 'prompt is too long: 1000123 tokens > 1000000 maximum' });
      return;
    }
    let body = checkpointAuthoringPromptsAnswered === 1 ? readCwdFile('checkpoint-author.md') : readCwdFile('checkpoint-author-2.md');
    if (body === null) {
      body = readCwdFile('checkpoint-author.md');
    }
    if (body === null) {
      body = '';
    }
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: body });
    writeEvent({ ev: 'turn', text: body });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  if (text.startsWith('[arx] checkpoint ') && text.includes('truth-check')) {
    checkpointTruthPromptsAnswered += 1;
    const usageLimitAt = process.env.ARX_STUB_USAGE_LIMIT_TRUTH_AT;
    if (usageLimitAt !== undefined && checkpointTruthPromptsAnswered === parseInt(usageLimitAt, 10)) {
      writeEvent({ ev: 'usage_limit', message: "You've reached your weekly limit" });
      return;
    }
    let reply = readCwdFile('checkpoint-truth.md');
    if (reply === null) {
      reply = 'ok';
    }
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: reply });
    writeEvent({ ev: 'turn', text: reply });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  if (text.startsWith('store:')) {
    const rest = text.slice('store:'.length);
    const sep = rest.indexOf(':');
    const verb = rest.slice(0, sep);
    const args = JSON.parse(rest.slice(sep + 1));
    const id = nextStoreId++;
    writeEvent({ ev: 'store', id, verb, args });
    const result = await awaitStoreResult(id);
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'stub:' });
    writeEvent({ ev: 'delta', text });
    writeEvent({ ev: 'turn', text: `stub-store:${JSON.stringify(result)}` });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  if (text.startsWith('file:')) {
    const rest = text.slice('file:'.length);
    const sep = rest.indexOf(':');
    const tool = rest.slice(0, sep);
    const args = JSON.parse(rest.slice(sep + 1));
    const id = nextFileId++;
    writeEvent({ ev: 'file', id, tool, args });
    const result = await awaitFileResult(id);
    await delayBeforeReply();
    writeEvent({ ev: 'delta', text: 'stub:' });
    writeEvent({ ev: 'delta', text });
    writeEvent({ ev: 'turn', text: `stub-file:${JSON.stringify(result)}` });
    writeEvent({ ev: 'usage', usage: currentUsage() });
    writeEvent({ ev: 'turn_end' });
    return;
  }
  await delayBeforeReply();
  writeEvent({ ev: 'delta', text: 'stub:' });
  writeEvent({ ev: 'delta', text });
  writeEvent({ ev: 'turn', text: `stub:${text}` });
  writeEvent({ ev: 'usage', usage: currentUsage() });
  writeEvent({ ev: 'turn_end' });
}
