import { mkdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { requestCore } from './core-client.mjs';
import { AislideClient } from '../packages/client/index.mjs';
import { startLocalModel } from './local-model-runtime.mjs';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

if (process.platform !== 'win32') throw new Error('This local qualification harness targets the prepared Windows runtime');
const directory = join('.artifacts', `real-model-${Date.now()}`);
await mkdir(directory, { recursive: true });
const model = await startLocalModel();
let mcp;
const keys = ['AISLIDE_AI_BASE_URL', 'AISLIDE_AI_MODEL', 'AISLIDE_AI_API_KEY', 'AISLIDE_AI_ALLOW_REMOTE', 'AISLIDE_AI_JSON_MODE', 'AISLIDE_AI_TIMEOUT_SECONDS'];
const previous = Object.fromEntries(keys.map((key) => [key, process.env[key]]));
try {
  Object.assign(process.env, model.environment);
  console.log('Real local Qwen2.5-1.5B server ready; requesting 12 slides through the unchanged shared core.');
  const input = {
    prompt: 'Create exactly 12 concise slides. Follow this outline in order: Synthetic quarterly review; Source and scope; Revenue by quarter; Reading the trend; Limits of the evidence; Data collection; Validation checks; Review workflow; Editable output; Quality review; Next steps; Source register. Use meaningful English titles, not language names. Use a cover, one chart with Q1-Q4 values 10,12,11,14, and short statement/process slides. Titles at most 36 characters and body strings at most 60. One short Japanese phrase on the cover is welcome. Do not invent facts. All numbers are synthetic. Output only report JSON.',
    source_text: 'SYNTHETIC TEST DATA, not factual business results. Quarter and revenue: Q1=10, Q2=12, Q3=11, Q4=14. Units are arbitrary synthetic units. Proposed actions, not observed facts: collect inputs, verify values, review results, export an editable copy. The data has not been independently fact-checked.',
    slide_count: 12, allow_remote: false, max_repairs: 1,
    outline: [
      ['Synthetic quarterly review', 'cover'], ['Source and scope', 'statement'], ['Revenue by quarter', 'chart'],
      ['Reading the trend', 'statement'], ['Limits of the evidence', 'statement'], ['Data collection', 'process'],
      ['Validation checks', 'statement'], ['Review workflow', 'process'], ['Editable output', 'statement'],
      ['Quality review', 'statement'], ['Next steps', 'statement'], ['Source register', 'statement'],
    ].map(([title, layout]) => ({ title, layout })),
  };
  let generated;
  if (process.argv.includes('--mcp')) {
    mcp = new Client({ name: 'real-model-poc', version: '1.0.0' });
    await mcp.connect(new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', resolve(directory)], env: { ...process.env }, stderr: 'pipe' }));
    const call = async (name, args) => {
      const response = await mcp.callTool({ name, arguments: args }, undefined, { timeout: 310000 });
      if (response.isError) throw new Error(response.content[0].text);
      return JSON.parse(response.content[0].text);
    };
    const result = await call('generate_report', input);
    const document = await call('get_document', { deck_id: result.deck_id });
    await call('update_text', { deck_id: result.deck_id, slide_id: 'slide-1', element_id: 'title', expected_revision: 0, text: 'Temporary MCP edit' });
    await call('undo', { deck_id: result.deck_id });
    await call('export_project', { deck_id: result.deck_id, filename: 'mcp-report.pptx' });
    generated = { report: document.report, compiled: { deck: document.deck, issues: result.issues }, provenance: result.provenance };
  } else generated = await requestCore({ op: 'generate', input });
  const client = new AislideClient(requestCore);
  const session = await client.createDocument({ id: 'real-local-model-proof', deck: generated.compiled.deck, report: generated.report });
  const layout = await requestCore({ op: 'measure_layout', deck: generated.compiled.deck });
  const exported = await session.exportProject();
  await writeFile(join(directory, 'model-result.json'), JSON.stringify(generated, null, 2), { flag: 'wx' });
  await writeFile(join(directory, 'report.pptx'), Buffer.from(exported.base64, 'base64'), { flag: 'wx' });
  await writeFile(join(directory, 'report.aislide.json'), JSON.stringify(exported.checkpoint), { flag: 'wx' });
  await writeFile(join(directory, 'layout.json'), JSON.stringify(layout, null, 2), { flag: 'wx' });
  const charts = generated.compiled.deck.slides.flatMap((slide) => slide.elements).filter((element) => element.type === 'chart');
  const chartValuesMatch = charts.length === 1 && JSON.stringify(charts[0].categories) === JSON.stringify(['Q1', 'Q2', 'Q3', 'Q4']) && charts[0].series.length === 1 && JSON.stringify(charts[0].series[0].values) === JSON.stringify([10, 12, 11, 14]);
  const summary = { directory, actual_model: 'Qwen2.5-1.5B-Instruct Q4_K_M', runtime: 'llama.cpp b10809', remote: false, fixture_response: false, via_mcp: Boolean(mcp), input_facts: 'explicitly synthetic with a supplied outline', slides: generated.compiled.deck.slides.length, chart_values_match: chartValuesMatch, provenance: generated.provenance, layout_errors: layout.issues.filter((issue) => issue.severity === 'error'), factual_review_complete: false, office_parity_verified: false };
  await writeFile(join(directory, 'qualification.json'), JSON.stringify(summary, null, 2), { flag: 'wx' });
  console.log(JSON.stringify(summary, null, 2));
  if (summary.layout_errors.length || !chartValuesMatch) process.exitCode = 1;
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ error: error.message, runtime: 'llama.cpp b10809', model: 'Qwen2.5-1.5B-Instruct Q4_K_M', remote: false }, null, 2));
  throw error;
} finally {
  await mcp?.close();
  for (const [key, value] of Object.entries(previous)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; }
  await model.stop();
  await writeFile(join(directory, 'server.log'), model.log);
}