import test from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { requestCore } from './core-client.mjs';
import { fixtureEnvironment, startProviderFixture } from './testing/provider-fixture.mjs';

test('real CLI generation cancels network work and accepts the next request', { timeout: 15_000 }, async () => {
  const report = await requestCore({ op: 'sample' });
  const fixture = await startProviderFixture(report);
  const environment = fixtureEnvironment(fixture.endpoint);
  const previous = Object.fromEntries(Object.keys(environment).map((key) => [key, process.env[key]]));
  Object.assign(process.env, environment);
  try {
    const generated = await requestCore({ op: 'generate', input: { prompt: 'Build a test report', slide_count: 3 } });
    assert.equal(generated.compiled.deck.slides.length, 3);
    assert.equal(generated.provenance.verified, false);
    await fixture.nextRequest();
    const cancellation = new AbortController();
    const generation = requestCore({ op: 'generate', input: { prompt: 'WAIT_FOR_CANCELLATION', slide_count: 3 } }, { signal: cancellation.signal });
    const failed = assert.rejects(generation, /cancel/i);
    const observed = await fixture.nextRequest();
    cancellation.abort();
    await failed;
    await observed.disconnected;
    assert.equal((await requestCore({ op: 'sample' })).sections.length, 12);
  } finally {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
    await fixture.close();
  }
});

test('MCP model generation returns a validated draft and never accepts endpoint overrides', { timeout: 15_000 }, async () => {
  const report = await requestCore({ op: 'sample' });
  const fixture = await startProviderFixture(report);
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], env: { ...process.env, ...fixtureEnvironment(fixture.endpoint) }, stderr: 'pipe' });
  const client = new Client({ name: 'generation-test', version: '1.0.0' });
  try {
    await client.connect(transport);
    const status = await client.callTool({ name: 'provider_status', arguments: {} });
    assert.ok(!status.isError, JSON.stringify(status.content));
    assert.equal(JSON.parse(status.content[0].text).model, 'local-fixture-model');
    const result = await client.callTool({ name: 'generate_report', arguments: { prompt: 'Build a test report', slide_count: 3 } });
    assert.ok(!result.isError, JSON.stringify(result.content));
    const data = JSON.parse(result.content[0].text);
    assert.equal(data.slides, 3);
    assert.equal(data.provenance.verified, false);
    const invalid = await client.callTool({ name: 'generate_report', arguments: { prompt: 'FAIL_INVALID_JSON', slide_count: 3 } });
    assert.equal(invalid.isError, true);
    const deck = await client.callTool({ name: 'get_deck', arguments: { deck_id: data.deck_id } });
    assert.equal(JSON.parse(deck.content[0].text).slides.length, 3);
    const extra = await client.callTool({ name: 'generate_report', arguments: { prompt: 'test', slide_count: 3, endpoint: 'http://example.invalid' } });
    assert.equal(extra.isError, true);
  } finally {
    await client.close();
    await fixture.close();
  }
});

test('one explicit validation repair is bounded and never becomes an implicit fallback', { timeout: 15000 }, async () => {
  const fixture = await startProviderFixture(await requestCore({ op: 'sample' }));
  const environment = { ...fixtureEnvironment(fixture.endpoint), AISLIDE_AI_JSON_MODE: 'schema' };
  const previous = Object.fromEntries(Object.keys(environment).map((key) => [key, process.env[key]]));
  Object.assign(process.env, environment);
  try {
    await assert.rejects(() => requestCore({ op: 'generate', input: { prompt: 'REPAIRABLE_JSON', slide_count: 3 } }), /model output is not valid/);
    assert.equal(fixture.receivedCount(), 1);
    const generated = await requestCore({ op: 'generate', input: { prompt: 'REPAIRABLE_JSON', slide_count: 3, max_repairs: 1 } });
    assert.equal(generated.provenance.attempts, 2);
    assert.equal(generated.compiled.deck.slides.length, 3);
    assert.equal(generated.provenance.verified, false);
    assert.equal(fixture.receivedCount(), 3);
    await assert.rejects(() => requestCore({ op: 'generate', input: { prompt: 'FAIL_INVALID_JSON', slide_count: 3, max_repairs: 1 } }), /model output is not valid/);
    assert.equal(fixture.receivedCount(), 5);
    const first = await fixture.nextRequest();
    assert.equal(first.payload.response_format.type, 'json_schema');
    assert.equal(first.payload.response_format.json_schema.schema.properties.sections.minItems, 3);
    assert.equal(first.payload.response_format.json_schema.schema.properties.sections.maxItems, 3);
    const incompatible = [{ title: 'Required different title', layout: 'statement' }, { title: 'Two', layout: 'statement' }, { title: 'Three', layout: 'statement' }];
    await assert.rejects(() => requestCore({ op: 'generate', input: { prompt: 'Outline mismatch', slide_count: 3, outline: incompatible } }), /approved outline/);
    assert.equal(fixture.receivedCount(), 6);
    const arraySchema = fixture.lastRequest().payload.response_format.json_schema.schema.properties.sections;
    assert.equal(arraySchema.prefixItems[0].properties.title.const, incompatible[0].title);
    assert.equal(arraySchema.prefixItems[0].properties.body.minItems, 1);
    assert.ok(!('items' in arraySchema), 'items must not mask prefixItems in compatible schema converters');
    await assert.rejects(() => requestCore({ op: 'generate', input: { prompt: 'REPAIRABLE_JSON', slide_count: 3, max_repairs: 2 } }));
    assert.equal(fixture.receivedCount(), 6);
  } finally {
    for (const [key, value] of Object.entries(previous)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; }
    await fixture.close();
  }
});