import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { drive, promptContent } from '../claude-main.mjs';

function assistantText(text) {
  return {
    type: 'assistant',
    parent_tool_use_id: null,
    message: { content: [{ type: 'text', text }] },
  };
}

function assistantToolUse(name) {
  return {
    type: 'assistant',
    parent_tool_use_id: null,
    message: { content: [{ type: 'tool_use', id: 't1', name, input: {} }] },
  };
}

function resultSuccess({ result, modelUsage = {} }) {
  return {
    type: 'result',
    subtype: 'success',
    is_error: false,
    result,
    usage: { input_tokens: 10, output_tokens: 5 },
    modelUsage,
  };
}

async function run(messages, options = {}) {
  const events = [];
  let closed = false;
  await drive(messages, {
    emit: (event) => events.push(event),
    close: () => {
      closed = true;
    },
    ...options,
  });
  return { events, closed };
}

test('a plain one-block turn emits the block text as the turn', async () => {
  const messages = [assistantText('hello'), resultSuccess({ result: 'hello', modelUsage: { 'claude-sonnet-5': { inputTokens: 10, outputTokens: 5 } } })];

  const { events } = await run(messages);

  const turn = events.filter((event) => event.ev === 'turn');
  assert.equal(turn.length, 1);
  assert.equal(turn[0].text, 'hello');
  const usage = events.find((event) => event.ev === 'usage');
  assert.deepEqual(usage.models, ['claude-sonnet-5']);
  assert.equal(events.filter((event) => event.ev === 'turn_end').length, 1);
});

test('a text, tool call, text turn joins the text blocks and ignores the last-block-only result', async () => {
  const messages = [
    assistantText('Let me check that.\n\n'),
    assistantToolUse('mcp__arx__decisions_query'),
    assistantText('Found 2 decisions.'),
    resultSuccess({ result: 'Found 2 decisions.', modelUsage: { 'claude-sonnet-5': { inputTokens: 40, outputTokens: 12 } } }),
  ];

  const { events } = await run(messages);

  const turn = events.filter((event) => event.ev === 'turn');
  assert.equal(turn.length, 1);
  assert.equal(turn[0].text, 'Let me check that.\n\nFound 2 decisions.');
  assert.equal(events.filter((event) => event.ev === 'turn_end').length, 1);
});

test('a turn with no assistant text falls back to the result text', async () => {
  const messages = [assistantToolUse('mcp__arx__decisions_query'), resultSuccess({ result: '', modelUsage: {} })];

  const { events } = await run(messages);

  const turn = events.filter((event) => event.ev === 'turn');
  assert.equal(turn.length, 1);
  assert.equal(turn[0].text, '');
  const usage = events.find((event) => event.ev === 'usage');
  assert.deepEqual(usage.models, []);
});

test('the accumulator resets between turns so a second turn does not carry the first one\'s text', async () => {
  const messages = [
    assistantText('first'),
    resultSuccess({ result: 'first', modelUsage: {} }),
    assistantText('second'),
    resultSuccess({ result: 'second', modelUsage: {} }),
  ];

  const { events } = await run(messages);

  const turns = events.filter((event) => event.ev === 'turn').map((event) => event.text);
  assert.deepEqual(turns, ['first', 'second']);
});

test('usage.models names every model whose token total grew this turn, cumulative across turns', async () => {
  const messages = [
    assistantText('one'),
    resultSuccess({ result: 'one', modelUsage: { a: { inputTokens: 100, outputTokens: 50 } } }),
    assistantText('two'),
    resultSuccess({
      result: 'two',
      modelUsage: { a: { inputTokens: 150, outputTokens: 60 }, b: { inputTokens: 10, outputTokens: 5 } },
    }),
    assistantText('three'),
    resultSuccess({
      result: 'three',
      modelUsage: { a: { inputTokens: 150, outputTokens: 60 }, b: { inputTokens: 10, outputTokens: 5 } },
    }),
  ];

  const { events } = await run(messages);

  const usageRows = events.filter((event) => event.ev === 'usage');
  assert.deepEqual(usageRows[0].models, ['a']);
  assert.deepEqual(usageRows[1].models, ['a', 'b']);
  assert.deepEqual(usageRows[2].models, []);
});

test('usage.models counts a model that grew only in cache tokens, with no new input or output tokens', async () => {
  const messages = [
    assistantText('one'),
    resultSuccess({
      result: 'one',
      modelUsage: { a: { inputTokens: 100, outputTokens: 50, cacheReadInputTokens: 0, cacheCreationInputTokens: 0 } },
    }),
    assistantText('two'),
    resultSuccess({
      result: 'two',
      modelUsage: {
        a: { inputTokens: 100, outputTokens: 50, cacheReadInputTokens: 4000, cacheCreationInputTokens: 0 },
        b: { inputTokens: 0, outputTokens: 0, cacheReadInputTokens: 0, cacheCreationInputTokens: 200 },
      },
    }),
  ];

  const { events } = await run(messages);

  const usageRows = events.filter((event) => event.ev === 'usage');
  assert.deepEqual(usageRows[0].models, ['a']);
  assert.deepEqual(usageRows[1].models, ['a', 'b']);
});

test('an error result emits fatal and calls close without emitting a turn', async () => {
  const messages = [{ type: 'result', subtype: 'error_during_execution', is_error: true, errors: ['boom'] }];

  const { events, closed } = await run(messages);

  assert.equal(events.length, 1);
  assert.equal(events[0].ev, 'fatal');
  assert.equal(events[0].message, 'boom');
  assert.equal(closed, true);
});

test('an interrupted turn marks turn_end stopped and consumes the flag once', async () => {
  const messages = [assistantText('partial'), resultSuccess({ result: 'partial', modelUsage: {} })];
  let interruptRequested = true;
  const consumeInterrupt = () => {
    const was = interruptRequested;
    interruptRequested = false;
    return was;
  };

  const { events } = await run(messages, { consumeInterrupt });

  const turn = events.find((event) => event.ev === 'turn');
  assert.equal(turn.text, 'partial');
  const turnEnd = events.find((event) => event.ev === 'turn_end');
  assert.equal(turnEnd.stopped, true);
});

test('an interrupted turn that comes back as is_error still ends the turn instead of going fatal', async () => {
  const messages = [
    assistantText('cut off mid'),
    { type: 'result', subtype: 'error_during_execution', is_error: true, errors: ['aborted'], usage: { input_tokens: 1, output_tokens: 1 }, modelUsage: {} },
  ];

  const { events, closed } = await run(messages, { consumeInterrupt: () => true });

  assert.equal(closed, false, 'an interrupted turn must not close the session');
  assert.equal(events.some((event) => event.ev === 'fatal'), false);
  const turnEnd = events.find((event) => event.ev === 'turn_end');
  assert.equal(turnEnd.stopped, true);
  const turn = events.find((event) => event.ev === 'turn');
  assert.equal(turn.text, 'cut off mid');
});

test('promptContent stays a plain string with no images, so an unattached prompt is untouched', () => {
  assert.equal(promptContent('hello', undefined), 'hello');
  assert.equal(promptContent('hello', []), 'hello');
});

test('promptContent builds a text block plus one base64 image block per attachment', () => {
  const dir = mkdtempSync(join(tmpdir(), 'arx-sidecar-test-'));
  const path = join(dir, '1.png');
  writeFileSync(path, Buffer.from([0x89, 0x50, 0x4e, 0x47]));

  const blocks = promptContent('what is in this image [image 1]', [{ path, media_type: 'image/png' }]);

  assert.deepEqual(blocks[0], { type: 'text', text: 'what is in this image [image 1]' });
  assert.equal(blocks[1].type, 'image');
  assert.equal(blocks[1].source.type, 'base64');
  assert.equal(blocks[1].source.media_type, 'image/png');
  assert.equal(blocks[1].source.data, Buffer.from([0x89, 0x50, 0x4e, 0x47]).toString('base64'));
});

test('a turn with no interrupt requested behaves exactly as before, byte for byte on the shape that matters', async () => {
  const messages = [assistantText('hello'), resultSuccess({ result: 'hello', modelUsage: {} })];

  const { events } = await run(messages);

  const turnEnd = events.find((event) => event.ev === 'turn_end');
  assert.equal('stopped' in turnEnd, false, 'an unstopped turn_end must carry no stopped key at all');
});


test('Codex tool export matches the Claude tool definitions', async () => {
  const { arxTools } = await import('../claude-main.mjs');
  const { z } = await import('zod');
  const { readFileSync } = await import('node:fs');
  const expected = arxTools.map(t => ({ name: t.name, description: t.description, inputSchema: z.toJSONSchema(z.object(t.inputSchema)) }));
  assert.deepEqual(JSON.parse(readFileSync(new URL('../main-tools.json', import.meta.url))), expected);
});


test('a canonical usage-limit result emits a typed usage_limit event', async () => {
  const events = [];
  await drive((async function* () { yield { type: 'result', subtype: 'success', is_error: true, result: "You've reached your weekly limit" }; })(), { emit: event => events.push(event), close: () => {} });
  assert.deepEqual(events, [{ ev: 'usage_limit', message: "You've reached your weekly limit" }]);
});

test('a generic error remains fatal', async () => {
  const events = [];
  await drive((async function* () { yield { type: 'result', subtype: 'error_during_execution', is_error: true, errors: ['provider broke'] }; })(), { emit: event => events.push(event), close: () => {} });
  assert.deepEqual(events, [{ ev: 'fatal', message: 'provider broke' }]);
});
