import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import test from 'node:test';

const workflows = ['issue-guidance.yml', 'pr-review.yml'];

function runScript(workflow, step) {
  const lines = readFileSync(new URL(`../.github/workflows/${workflow}`, import.meta.url), 'utf8').split('\n');
  const start = lines.indexOf(`      - name: ${step}`);
  assert.ok(start >= 0, `Missing step: ${step}`);
  const run = lines.findIndex((line, index) => index > start && line === '        run: |');
  assert.ok(run >= 0, `Missing script: ${step}`);
  const body = [];
  for (const line of lines.slice(run + 1)) {
    if (line.trim() && !line.startsWith('          ')) break;
    body.push(line.slice(10));
  }
  return body.join('\n');
}

function fixture(t, overrides = {}) {
  const directory = mkdtempSync(join(tmpdir(), 'aislide-copilot-workflows-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const bin = join(directory, 'bin');
  mkdirSync(bin);
  mkdirSync(join(directory, '.github'));
  writeFileSync(join(directory, '.github', 'copilot-instructions.md'), 'Test project instructions.\n');
  const calls = join(directory, 'gh-calls.txt');
  const args = join(directory, 'copilot-args.txt');
  const count = join(directory, 'copilot-count.txt');
  for (const file of [calls, args, count]) writeFileSync(file, '');
  writeFileSync(join(bin, 'git'), '#!/usr/bin/env bash\nprintf "crates/aislide-core/src/lib.rs\\nREADME.md\\n"\n', { mode: 0o755 });
  writeFileSync(join(bin, 'gh'), `#!/usr/bin/env bash
set -euo pipefail
printf '%s\\t%s\\n' "$GH_TOKEN" "$*" >> "$MOCK_GH_CALLS"
case "$*" in
  'api user --jq .login')
    [ "$GH_TOKEN" = 'copilot-token' ] || exit 9
    if [ "$MOCK_OWNER_EXIT" != '0' ]; then exit "$MOCK_OWNER_EXIT"; fi
    printf '%s\\n' "$MOCK_TOKEN_OWNER" ;;
  'label create '*|'issue comment '*|'issue edit '*|'pr comment '*) ;;
  'api repos/murasamelabo/AISlide/issues?state=open&per_page=100 --paginate')
    printf '[{"number":17,"labels":[]},{"number":18,"labels":[]}]\\n' ;;
  'api repos/murasamelabo/AISlide/branches --paginate --jq .[].name') ;;
  'api graphql '*) printf '0\\n' ;;
  'issue view '*) printf '{"state":"OPEN","labels":[],"title":"Test","body":"Test issue."}\\n' ;;
  'pr view '*)
    if [[ "$*" == *'--json headRefOid'* ]]; then
      printf '%s\\n' "$MOCK_CURRENT_HEAD"
    else
      printf 'Title: Test PR\\nDescription: Test change.\\n'
    fi ;;
  'pr diff '*) printf 'diff --git a/README.md b/README.md\\n+Test change.\\n' ;;
  'pr checks '*) printf 'Verify pass\\n' ;;
  *) printf 'Unexpected gh call: %s\\n' "$*" >&2; exit 8 ;;
esac
`, { mode: 0o755 });
  writeFileSync(join(bin, 'copilot'), `#!/usr/bin/env bash
set -euo pipefail
printf 'call\\n' >> "$MOCK_COPILOT_COUNT"
while [ "$#" -gt 0 ]; do
  if [ "$1" = '-p' ]; then shift 2; else
    printf '%s\\n' "$1" >> "$MOCK_COPILOT_ARGS"
    shift
  fi
done
if [ "$MOCK_COPILOT_MODE" = 'fail' ] || { [ "$MOCK_COPILOT_MODE" = 'fail-first' ] && [ "$(wc -l < "$MOCK_COPILOT_COUNT")" -eq 1 ]; }; then
  echo 'Error: Access denied by policy settings' >&2
  exit 1
fi
if [ "$MOCK_COPILOT_MODE" != 'empty' ]; then
  printf '判定: マージ可\\nTest response.\\n'
fi
`, { mode: 0o755 });
  const env = {
    PATH: `${bin}${delimiter}${process.env.PATH}`,
    HOME: directory,
    TMPDIR: directory,
    GH_TOKEN: 'repository-token',
    COPILOT_GITHUB_TOKEN: 'copilot-token',
    REPO: 'murasamelabo/AISlide',
    DEVELOPER: 'murasamelabo',
    LABEL: 'ai-guidance',
    EVENT_NAME: 'issues',
    EVENT_ISSUE_NUMBER: '17',
    PR_NUMBER: '23',
    HEAD_SHA: 'expected-head',
    MOCK_GH_CALLS: calls,
    MOCK_COPILOT_ARGS: args,
    MOCK_COPILOT_COUNT: count,
    MOCK_TOKEN_OWNER: 'murasamelabo',
    MOCK_OWNER_EXIT: '0',
    MOCK_CURRENT_HEAD: 'expected-head',
    MOCK_COPILOT_MODE: 'success',
    ...overrides,
  };
  return {
    run(workflow, step) {
      const source = readFileSync(new URL(`../.github/workflows/${workflow}`, import.meta.url), 'utf8');
      const model = source.match(/^      AI_MODEL: (.+)$/m);
      assert.ok(model, `Missing model in ${workflow}`);
      const result = spawnSync('bash', ['-e', '-c', runScript(workflow, step)], { cwd: directory, env: { ...env, AI_MODEL: model[1] }, encoding: 'utf8', timeout: 15000 });
      assert.ifError(result.error);
      return { ...result, calls: readFileSync(calls, 'utf8'), args: readFileSync(args, 'utf8'), count: readFileSync(count, 'utf8').trim().split('\n').filter(Boolean).length };
    },
  };
}

for (const workflow of workflows) {
  test(`${workflow}: pins GPT 6.1 Sol without a repository variable override`, () => {
    const source = readFileSync(new URL(`../.github/workflows/${workflow}`, import.meta.url), 'utf8');
    assert.match(source, /^      AI_MODEL: gpt-6\.1-sol$/m);
    assert.doesNotMatch(source, /vars\.AI_MODEL/);
  });
  test(`${workflow}: dedicated secret only`, () => {
    const source = readFileSync(new URL(`../.github/workflows/${workflow}`, import.meta.url), 'utf8');
    assert.match(source, /COPILOT_GITHUB_TOKEN: \$\{\{ secrets\.MURASAMELABO_COPILOT_TOKEN \}\}/);
    assert.doesNotMatch(source, /secrets\.COPILOT_GITHUB_TOKEN/);
  });
  test(`${workflow}: missing token fails before any API call`, (t) => {
    const result = fixture(t, { COPILOT_GITHUB_TOKEN: '' }).run(workflow, 'Check Copilot account');
    assert.equal(result.status, 1);
    assert.match(result.stdout, /::error::Secret MURASAMELABO_COPILOT_TOKEN is not set/);
    assert.equal(result.calls, '');
  });
  test(`${workflow}: rejects another account`, (t) => {
    const result = fixture(t, { MOCK_TOKEN_OWNER: 'previous-account' }).run(workflow, 'Check Copilot account');
    assert.equal(result.status, 1);
    assert.match(result.stdout, /must belong to murasamelabo, not previous-account/);
    assert.match(result.calls, /^copilot-token\tapi user --jq \.login\n$/);
  });
  for (const exit of ['1', '124']) {
    test(`${workflow}: failed or timed-out owner lookup (${exit}) fails explicitly`, (t) => {
      const result = fixture(t, { MOCK_OWNER_EXIT: exit }).run(workflow, 'Check Copilot account');
      assert.equal(result.status, 1);
      assert.match(result.stdout, /::error::Cannot verify the Copilot token owner/);
    });
  }
  test(`${workflow}: accepts murasamelabo without reusing the repository token`, (t) => {
    const result = fixture(t).run(workflow, 'Check Copilot account');
    assert.equal(result.status, 0);
    assert.equal(result.calls, 'copilot-token\tapi user --jq .login\n');
  });
}

test('guidance: successful requests use GPT 6.1 Sol and the repository token for writes', (t) => {
  const result = fixture(t, { AI_MODEL: 'previous-model' }).run('issue-guidance.yml', 'Post guidance for not-started issues');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.args, /--model\ngpt-6\.1-sol\n/);
  assert.doesNotMatch(result.args, /previous-model/);
  assert.match(result.calls, /repository-token\tissue comment 17 /);
  assert.match(result.calls, /repository-token\tissue edit 17 .*--add-label ai-guidance/);
});

for (const mode of ['fail', 'empty']) {
  test(`guidance: ${mode} responses cannot produce a successful run or label`, (t) => {
    const result = fixture(t, { MOCK_COPILOT_MODE: mode, EVENT_NAME: 'schedule' }).run('issue-guidance.yml', 'Post guidance for not-started issues');
    assert.equal(result.status, 1);
    assert.equal(result.count, 2);
    assert.match(result.stdout, /::error::Guidance failed for 2 issue\(s\)/);
    assert.doesNotMatch(result.calls, /issue (comment|edit)/);
  });
}

test('guidance: other issues still succeed after a failure, but the job fails', (t) => {
  const result = fixture(t, { MOCK_COPILOT_MODE: 'fail-first', EVENT_NAME: 'schedule' }).run('issue-guidance.yml', 'Post guidance for not-started issues');
  assert.equal(result.status, 1);
  assert.equal(result.count, 2);
  assert.doesNotMatch(result.calls, /issue (comment|edit) 17 /);
  assert.match(result.calls, /issue comment 18 /);
  assert.match(result.calls, /issue edit 18 .*--add-label ai-guidance/);
});

test('review: uses GPT 6.1 Sol and the repository token without approving or merging', (t) => {
  const result = fixture(t, { AI_MODEL: 'previous-model' }).run('pr-review.yml', 'Review pull request');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.args, /--model\ngpt-6\.1-sol\n/);
  assert.doesNotMatch(result.args, /previous-model/);
  assert.match(result.calls, /repository-token\tpr comment 23 /);
  assert.doesNotMatch(result.calls, /pr (review|merge)/);
  for (const tool of ['shell', 'write', 'url', 'read', 'memory']) assert.ok(result.args.includes(`--deny-tool=${tool}\n`));
  assert.ok(result.args.includes('--disable-builtin-mcps\n'));
});

for (const mode of ['fail', 'empty']) {
  test(`review: ${mode} responses fail without posting a comment`, (t) => {
    const result = fixture(t, { MOCK_COPILOT_MODE: mode }).run('pr-review.yml', 'Review pull request');
    assert.equal(result.status, 1);
    assert.match(result.stdout, /::error::Copilot CLI/);
    assert.doesNotMatch(result.calls, /pr comment/);
  });
}

test('review: moved heads skip without a Copilot request or stale comment', (t) => {
  const result = fixture(t, { MOCK_CURRENT_HEAD: 'new-head' }).run('pr-review.yml', 'Review pull request');
  assert.equal(result.status, 0);
  assert.equal(result.count, 0);
  assert.match(result.stdout, /::notice::PR head moved/);
  assert.doesNotMatch(result.calls, /pr comment/);
});
