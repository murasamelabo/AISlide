import { createServer } from 'node:http';

export async function startProviderFixture(report) {
  const waiting = [];
  const received = [];
  let requestCount = 0;
  let lastRequest;
  const server = createServer(async (request, response) => {
    try {
      let bytes = '';
      for await (const chunk of request) bytes += chunk;
      const payload = JSON.parse(bytes);
      const input = JSON.parse(payload.messages[1].content);
      const observed = { payload, input, disconnected: new Promise((resolve) => response.on('close', resolve)) };
      requestCount += 1;
      lastRequest = observed;
      if (waiting.length) waiting.shift()(observed);
      else received.push(observed);
      if (input.brief.includes('WAIT_FOR_CANCELLATION')) return;
      if (input.brief.includes('FAIL_INVALID_JSON') || (input.brief.includes('REPAIRABLE_JSON') && !input.validation_feedback)) {
        response.writeHead(200, { 'Content-Type': 'application/json' });
        response.end(JSON.stringify({ choices: [{ finish_reason: 'stop', message: { content: 'not a report' } }] }));
        return;
      }
      const draft = structuredClone(report);
      draft.title = 'Fixture-generated analysis';
      draft.sections = draft.sections.slice(0, input.slide_count);
      draft.sections[0].title = 'Fixture-generated analysis';
      draft.source = 'Synthetic local test fixture; not output from a real model.';
      response.writeHead(200, { 'Content-Type': 'application/json' });
      response.end(JSON.stringify({ choices: [{ finish_reason: 'stop', message: { content: JSON.stringify(draft) } }] }));
    } catch {
      response.writeHead(400).end();
    }
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  return {
    endpoint: `http://127.0.0.1:${server.address().port}/v1`,
    nextRequest: () => received.length ? Promise.resolve(received.shift()) : new Promise((resolve) => waiting.push(resolve)),
    receivedCount: () => requestCount,
    lastRequest: () => lastRequest,
    close: () => new Promise((resolve) => { server.close(resolve); server.closeAllConnections(); }),
  };
}

export function fixtureEnvironment(endpoint) {
  return {
    AISLIDE_AI_BASE_URL: endpoint,
    AISLIDE_AI_MODEL: 'local-fixture-model',
    AISLIDE_AI_API_KEY: '',
    AISLIDE_AI_ALLOW_REMOTE: '0',
    AISLIDE_AI_JSON_MODE: '1',
    AISLIDE_AI_TIMEOUT_SECONDS: '60',
  };
}