import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

const workflow = await readFile(new URL('../.github/workflows/pr-review.yml', import.meta.url), 'utf8');
function script(name) {
  const start = workflow.indexOf(`      - name: ${name}\n`);
  assert.notEqual(start, -1);
  const block = workflow.slice(start).split(/\n      - (?:name:|uses:)/)[0];
  const lines = [];
  for (const line of block.split('        run: |\n')[1].split('\n')) {
    if (line && !line.startsWith('          ')) break;
    lines.push(line.replace(/^          /, ''));
  }
  return lines.join('\n');
}

const pr = {
  number: 50, state: 'open', draft: false,
  head: { sha: 'abc123', repo: { full_name: 'owner/repo' } },
  base: { repo: { full_name: 'owner/repo' } },
  user: { login: 'murasamelabo' }, mergeable: true,
  requested_reviewers: [{ login: 'reviewer' }], requested_teams: [],
};
const verify = {
  id: 123, run_number: 10, run_attempt: 1, status: 'completed', conclusion: 'success',
  html_url: 'https://github.com/owner/repo/actions/runs/123',
};

async function run(context, name, options = {}) {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-pr-review-'));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const state = {
    pr: structuredClone(pr), run: structuredClone(verify), verdict: '判定: マージ可',
    event: { pull_request: pr, workflow_run: { head_sha: 'abc123', pull_requests: [] } },
    ...options,
  };
  await writeFile(join(directory, 'state.json'), JSON.stringify(state));
  await writeFile(join(directory, 'event.json'), JSON.stringify(state.event));
  await writeFile(join(directory, 'output'), '');
  // Execute the workflow's actual shell with only external services mocked.
  await writeFile(join(directory, 'gh'), `#!/usr/bin/env node
import { readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
const state = JSON.parse(readFileSync(process.env.STATE, 'utf8'));
const args = process.argv.slice(2);
appendFileSync(process.env.CALLS, JSON.stringify({ args, token: process.env.GH_TOKEN }) + '\\n');
if (args[0] === 'api') {
  const endpoint = args.find(arg => arg.startsWith('repos/'));
  let value;
  if (endpoint.includes('/commits/')) value = [state.pr];
  else if (endpoint.includes('/comments')) value = state.comments || [];
  else if (endpoint.endsWith('/pulls/50')) value = state.finalPr || state.pr;
  else if (endpoint.includes('/jobs')) value = { jobs: [{ name: 'windows', conclusion: state.run.conclusion, status: 'completed', html_url: 'https://example.test/job' }] };
  else if (endpoint.includes('/runs?')) value = { workflow_runs: state.runs || [state.run] };
  else if (endpoint.endsWith('/runs/123')) value = state.run;
  else throw new Error('Unexpected API: ' + endpoint);
  const query = args.indexOf('--jq');
  if (query >= 0) {
    const result = spawnSync('jq', ['-r', args[query + 1]], { input: JSON.stringify(value), encoding: 'utf8' });
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
    process.exit(result.status);
  }
  console.log(JSON.stringify(value));
} else if (args[0] === 'pr' && args[1] === 'view') {
  console.log(state.moved ? 'new-head' : state.pr.head.sha);
} else if (args[0] === 'pr' && args[1] === 'diff') {
  if (state.diffFailed) process.exit(1);
  console.log(state.largeDiff ? 'x'.repeat(81000) : 'synthetic test diff');
} else if (args[0] === 'pr' && args[1] === 'review') {
  process.exit(state.approvalFailed ? 1 : 0);
} else if (args[0] === 'pr' && args[1] === 'merge') {
  process.exit(state.mergeFailed || (state.fallback && !args.includes('--auto')) ? 1 : 0);
} else if (args[0] === 'pr' && args[1] === 'comment') {
  writeFileSync(process.env.COMMENT, readFileSync(args[args.indexOf('--body-file') + 1]));
} else throw new Error('Unexpected command: ' + args.join(' '));
`, { mode: 0o755 });
  await writeFile(join(directory, 'copilot'), `#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
const args = process.argv.slice(2);
writeFileSync(process.env.PROMPT, args[args.indexOf('-p') + 1]);
console.log(JSON.parse(readFileSync(process.env.STATE, 'utf8')).verdict + '\\nSynthetic review');
`, { mode: 0o755 });
  const result = spawnSync('bash', ['-c', script(name)], {
    cwd: new URL('..', import.meta.url),
    encoding: 'utf8',
    env: {
      ...process.env, PATH: `${directory}:${process.env.PATH}`,
      STATE: join(directory, 'state.json'), CALLS: join(directory, 'calls'),
      COMMENT: join(directory, 'comment'), PROMPT: join(directory, 'prompt'),
      GH_TOKEN: 'synthetic-comment-token', MERGE_TOKEN: 'synthetic-merge-token',
      REPO: 'owner/repo', PR_NUMBER: '50', HEAD_SHA: 'abc123', VERIFY_RUN_ID: '123',
      PR_REVIEW_MODEL: 'claude-opus-5.5', DEVELOPER: 'murasamelabo',
      AUTO_MERGE_ALLOWED_AUTHORS: 'murasamelabo,Copilot,copilot-swe-agent[bot]',
      AI_AUTO_MERGE: '', GITHUB_EVENT_NAME: 'workflow_run',
      GITHUB_EVENT_PATH: join(directory, 'event.json'), GITHUB_OUTPUT: join(directory, 'output'),
      TMPDIR: directory, ...options.env,
    },
  });
  assert.equal(result.status, 0, result.stderr);
  const optional = path => readFile(join(directory, path), 'utf8').catch(error => {
    if (error.code === 'ENOENT') return '';
    throw error;
  });
  return {
    calls: (await optional('calls')).trim().split('\n').filter(Boolean).map(line => JSON.parse(line)),
    comment: await optional('comment'), prompt: await optional('prompt'),
    output: await optional('output'), stdout: result.stdout,
  };
}

const reviews = result => result.calls.filter(call => call.args[1] === 'review');
const merges = result => result.calls.filter(call => call.args[1] === 'merge');

test('review_requested and Verify completion use PR-scoped serialization and trusted checkout', () => {
  assert.match(workflow, /types: \[review_requested\]/);
  assert.match(workflow, /workflows: \[Verify\]\n    types: \[completed\]/);
  assert.match(workflow, /workflow_run\.event == 'pull_request'/);
  assert.match(workflow, /group: pr-review-\$\{\{ matrix.number \}\}\n      cancel-in-progress: false/);
  assert.match(workflow, /ref: \$\{\{ github.event.repository.default_branch \}\}/);
  for (const tool of ['shell', 'write', 'url', 'read', 'memory']) {
    assert.ok(workflow.includes(`--deny-tool=${tool}`));
  }
});

test('workflow_run resolves fork PRs through commit pulls and ignores stale/closed/draft/unrequested PRs', async context => {
  const result = await run(context, 'Resolve requested pull requests');
  assert.deepEqual(JSON.parse(result.output.split('=')[1]), [{ number: 50, sha: 'abc123' }]);
  assert.ok(result.calls.some(call => call.args.some(arg => arg.includes('/commits/abc123/pulls'))));
  for (const patch of [
    { state: 'closed' }, { draft: true }, { requested_reviewers: [] },
    { head: { ...pr.head, sha: 'old-head' } }, { base: { repo: { full_name: 'other/repo' } } },
  ]) {
    assert.equal((await run(context, 'Resolve requested pull requests', { pr: { ...pr, ...patch } })).output, 'targets=[]\n');
  }
});

test('review_requested resolves direct PR and team requests qualify', async context => {
  const result = await run(context, 'Resolve requested pull requests', {
    env: { GITHUB_EVENT_NAME: 'pull_request_target' },
    pr: { ...pr, requested_reviewers: [], requested_teams: [{ slug: 'team' }] },
  });
  assert.match(result.output, /"number":50/);
  assert.ok(!result.calls.some(call => call.args.some(arg => arg.includes('/commits/'))));
});

test('gate waits for the latest Verify, including when an older run succeeded', async context => {
  for (const runs of [[], [{ ...verify, status: 'in_progress', conclusion: null }], [
    verify, { ...verify, id: 124, run_number: 11, status: 'queued', conclusion: null },
  ]]) {
    const result = await run(context, 'Wait for Verify and skip duplicate reviews', { runs });
    assert.equal(result.output, '');
    assert.match(result.stdout, /Verify 完了後/);
  }
  assert.match((await run(context, 'Wait for Verify and skip duplicate reviews')).output, /ready=true/);
});

test('gate skips bot-marked duplicate heads but not user-forged markers or other heads', async context => {
  for (const [login, sha, skip] of [
    ['github-actions[bot]', 'abc123', true], ['contributor', 'abc123', false],
    ['github-actions[bot]', 'older-head', false],
  ]) {
    const result = await run(context, 'Wait for Verify and skip duplicate reviews', {
      comments: [{ user: { login }, body: `<!-- ai-pr-review head=${sha} -->` }],
    });
    assert.equal(result.output.includes('ready=true'), !skip);
  }
});

test('successful eligible verdict approves then merges with the exact head and separate token', async context => {
  const result = await run(context, 'Review pull request');
  assert.equal(reviews(result).length, 1);
  assert.ok(reviews(result)[0].args.includes('--approve'));
  assert.equal(merges(result).length, 1);
  assert.deepEqual(merges(result)[0].args.slice(-3), ['--squash', '--match-head-commit', 'abc123']);
  assert.equal(merges(result)[0].token, 'synthetic-merge-token');
  assert.ok(result.comment.includes('<!-- ai-pr-review head=abc123 -->'));
  assert.match(result.comment, /@murasamelabo/);
  assert.match(result.comment, /claude-opus-5.5/);
  assert.ok(result.comment.includes(verify.html_url));
  assert.match(result.prompt, /Verify conclusion: success/);
  assert.match(result.prompt, /windows: success/);
});

test('protected branches fall back to auto and approval failures never attempt merging', async context => {
  const fallback = await run(context, 'Review pull request', { fallback: true });
  assert.equal(merges(fallback).length, 2);
  assert.ok(merges(fallback)[1].args.includes('--auto'));
  assert.ok(merges(fallback)[1].args.includes('--match-head-commit'));
  assert.match(fallback.comment, /予約/);
  const failed = await run(context, 'Review pull request', { approvalFailed: true });
  assert.equal(merges(failed).length, 0);
  assert.match(failed.comment, /承認に失敗/);
  assert.match((await run(context, 'Review pull request', { mergeFailed: true })).comment, /要求が失敗/);
});

test('negative verdicts request changes but never merge', async context => {
  for (const verdict of ['判定: 修正後マージ可', '判定: マージ不可']) {
    for (const conclusion of ['success', 'failure']) {
      const result = await run(context, 'Review pull request', { verdict, run: { ...verify, conclusion } });
      assert.ok(reviews(result)[0].args.includes('--request-changes'));
      assert.equal(merges(result).length, 0);
      if (conclusion === 'failure') assert.match(result.comment, /Verify が failure/);
    }
  }
});

test('non-success Verify and malformed verdicts fail closed', async context => {
  for (const conclusion of ['failure', 'cancelled', 'timed_out', 'action_required', 'neutral', 'skipped', null]) {
    const result = await run(context, 'Review pull request', { run: { ...verify, conclusion } });
    assert.equal(reviews(result).length, 0);
    assert.equal(merges(result).length, 0);
    assert.match(result.comment, /Verify が/);
    assert.ok(result.comment.includes(`windows: ${conclusion ?? 'completed'}`));
  }
  const unknown = await run(context, 'Review pull request', { verdict: '判定: マージ可 (probably)' });
  assert.equal(reviews(unknown).length, 0);
  assert.equal(merges(unknown).length, 0);
  assert.match(unknown.comment, /認識できません/);
});

test('forks, disallowed authors and disabled automation receive comments only, even for negative verdicts', async context => {
  for (const verdict of ['判定: マージ可', '判定: マージ不可']) {
    for (const options of [
      { pr: { ...pr, head: { ...pr.head, repo: { full_name: 'fork/repo' } } } },
      { pr: { ...pr, user: { login: 'outsider' } } },
      { env: { AI_AUTO_MERGE: 'false' } },
    ]) {
      const result = await run(context, 'Review pull request', { verdict, ...options });
      assert.equal(reviews(result).length, 0);
      assert.equal(merges(result).length, 0);
      assert.match(result.comment, /保留/);
    }
  }
});

test('unknown mergeability, missing/truncated diffs, changed head, and restarted Verify prevent merging', async context => {
  for (const options of [
    { pr: { ...pr, mergeable: null } }, { diffFailed: true }, { largeDiff: true },
    { moved: true }, { finalPr: { ...pr, requested_reviewers: [] } },
    { run: { ...verify, status: 'in_progress', conclusion: null } },
    { runs: [{ ...verify, status: 'in_progress', conclusion: null }] },
    { runs: [{ ...verify, run_attempt: 2 }] },
    { runs: [{ ...verify, id: 124, run_number: 11 }] },
  ]) {
    const result = await run(context, 'Review pull request', options);
    assert.equal(reviews(result).length, 0);
    assert.equal(merges(result).length, 0);
  }
});
