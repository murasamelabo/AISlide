import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';

const root = resolve(import.meta.dirname, '..');
const validator = join(root, 'tools/validate-openxml.ps1');
const cli = join(root, 'target/debug', process.platform === 'win32' ? 'aislide.exe' : 'aislide');
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const knownIds = ['Sch_ElementValueDataTypeDetailed', 'Sch_UndeclaredAttribute'];
const evidence = [];

function validate(path, compatibility = false, targetVersion) {
  const before = hash(path);
  const result = spawnSync('pwsh', ['-NoProfile', '-NonInteractive', '-File', validator,
    '-Path', path, ...(compatibility ? ['-AllowOfficeHistogramBinning'] : []),
    ...(targetVersion ? ['-TargetVersion', targetVersion] : [])],
  { cwd: root, encoding: 'utf8', windowsHide: true, maxBuffer: 8 * 1024 * 1024, env: { ...process.env, NO_COLOR: '1', TERM: 'dumb' } });
  assert.ifError(result.error);
  assert.equal(result.signal, null);
  assert.equal(hash(path), before, 'validation must leave every source byte unchanged');
  const records = result.stdout.split(/\r?\n/).filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const summary = records.find(record => 'GateAccepted' in record);
  evidence.push({ name: basename(path), sha256: before, compatibility, targetVersion: targetVersion ?? 'Office2016', exit: result.status, summary });
  return { ...result, issues: records.filter(record => 'Description' in record), summary };
}

function assertSummary(result, { compatibility, known = 0, unexpected = 0, limited = false, targetVersion = 'Office2016' }) {
  const count = known + unexpected;
  const accepted = !limited && unexpected === 0 && (count === 0 || compatibility);
  assert.deepEqual(result.summary, {
    Mode: compatibility ? 'OfficeHistogramBinningCompatibility' : 'Strict',
    TargetVersion: targetVersion,
    SchemaValid: count === 0 && !limited,
    IssueCount: count,
    KnownCompatibilityIssueCount: known,
    UnexpectedIssueCount: unexpected,
    IssueLimitReached: limited,
    GateAccepted: accepted,
    CompatibilityAccepted: accepted && known > 0 && compatibility,
  }, result.stdout + result.stderr);
  assert.equal(result.status, accepted ? 0 : 1, result.stdout + result.stderr);
  assert.equal(result.issues.length, count);
  assert.equal(result.issues.filter(issue => issue.Classification === 'OfficeHistogramBinning').length, known);
  for (const issue of result.issues) {
    assert.ok(issue.Description);
    assert.ok(issue.Id);
    assert.ok(issue.Part);
    assert.ok(issue.XPath);
    assert.equal(issue.Severity, compatibility && issue.Classification === 'OfficeHistogramBinning' ? 'Warning' : 'Error');
  }
  if (known > 0 && compatibility) assert.match(result.stderr, /known schema deviations.*not schema-valid; not visual parity/i);
  else assert.doesNotMatch(result.stderr, /WARNING:/);
  if (count > 0) assert.doesNotMatch(result.stdout, /schema validation passed/i);
  if (!accepted) assert.doesNotMatch(result.stdout, /compatibility validation accepted/i);
  assert.doesNotMatch(result.stdout, /WARNING:/);
}

function mutate(source, destination, part, mutation, value = '') {
  const before = hash(source);
  const script = `
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[System.IO.File]::Copy($env:HIST_SOURCE, $env:HIST_DESTINATION, $false)
$archive = [System.IO.Compression.ZipFile]::Open($env:HIST_DESTINATION, [System.IO.Compression.ZipArchiveMode]::Update)
try {
    $entry = $archive.GetEntry($env:HIST_PART.TrimStart('/'))
    if ($null -eq $entry) { throw 'Missing synthetic chart part' }
    $xml = [System.Xml.XmlDocument]::new()
    $xml.PreserveWhitespace = $true
    $xml.XmlResolver = $null
    $settings = [System.Xml.XmlReaderSettings]::new()
    $settings.DtdProcessing = [System.Xml.DtdProcessing]::Prohibit
    $settings.XmlResolver = $null
    $stream = $entry.Open()
    $reader = [System.Xml.XmlReader]::Create($stream, $settings)
    try { $xml.Load($reader) } finally { $reader.Dispose(); $stream.Dispose() }
    $namespaces = [System.Xml.XmlNamespaceManager]::new($xml.NameTable)
    $namespace = 'http://schemas.microsoft.com/office/drawing/2014/chartex'
    $namespaces.AddNamespace('cx', $namespace)
    $series = $xml.SelectSingleNode('/cx:chartSpace/cx:chart/cx:plotArea/cx:plotAreaRegion/cx:series[1]', $namespaces)
    $binning = $series.SelectSingleNode('cx:layoutPr/cx:binning', $namespaces)
    $node = $binning.SelectSingleNode('cx:binCount | cx:binSize', $namespaces)
    if ($null -eq $node) { throw 'Missing synthetic histogram node' }
    switch ($env:HIST_MUTATION) {
        'value' { $node.SetAttribute('val', $env:HIST_VALUE) }
        'extra-attribute' { $node.SetAttribute('unexpected', '1') }
        'qualified-val' {
            $node.RemoveAttribute('val')
            $null = $node.SetAttribute('val', 'urn:aislide:test:foreign', '4')
        }
        'mixed-text' { $node.InnerText = '4' }
        'whitespace-text' { $node.InnerText = ' ' }
        'nested-child' { $null = $node.AppendChild($xml.CreateElement('cx', 'unexpected', $namespace)) }
        'duplicate-value' { $null = $binning.AppendChild($node.CloneNode($true)) }
        'both-values' {
            $other = $xml.CreateElement('cx', 'binSize', $namespace)
            $other.SetAttribute('val', '2.5')
            $null = $binning.AppendChild($other)
        }
        'wrong-layout' { $series.SetAttribute('layoutId', 'waterfall') }
        'missing-layout' { $series.RemoveAttribute('layoutId') }
        'wrong-parent' { $null = $binning.ParentNode.AppendChild($node) }
        'wrong-ancestor' { $null = $series.AppendChild($binning) }
        'wrong-namespace' {
            $other = $xml.CreateElement('foreign', 'binCount', 'urn:aislide:test:foreign')
            $other.SetAttribute('val', '4')
            $null = $binning.ReplaceChild($other, $node)
        }
        'invalid-interval' { $binning.SetAttribute('intervalClosed', 'invalid') }
        'unrelated-error' { $xml.DocumentElement.SetAttribute('unexpected', '1') }
        'text-encoding' { $node.InnerText = $node.GetAttribute('val'); $node.RemoveAttribute('val') }
        'many-series' {
            for ($index = 1; $index -lt [int]$env:HIST_VALUE; $index++) { $null = $series.ParentNode.AppendChild($series.CloneNode($true)) }
        }
        default { throw 'Unknown synthetic mutation' }
    }
    $name = $entry.FullName
    $entry.Delete()
    $output = $archive.CreateEntry($name).Open()
    $writerSettings = [System.Xml.XmlWriterSettings]::new()
    $writerSettings.Encoding = [System.Text.UTF8Encoding]::new($false)
    $writer = [System.Xml.XmlWriter]::Create($output, $writerSettings)
    try { $xml.Save($writer) } finally { $writer.Dispose(); $output.Dispose() }
} finally { $archive.Dispose() }
`;
  const result = spawnSync('pwsh', ['-NoProfile', '-NonInteractive', '-Command', script], {
    cwd: root, encoding: 'utf8', windowsHide: true,
    env: { ...process.env, HIST_SOURCE: source, HIST_DESTINATION: destination, HIST_PART: part, HIST_MUTATION: mutation, HIST_VALUE: value },
  });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.equal(hash(source), before, 'fixture mutations must not change their source');
  return destination;
}

test('Open XML histogram compatibility remains explicit and never claims schema validity', async context => {
  const directory = mkdtempSync(join(tmpdir(), 'aislide-histogram-validation-'));
  const cliBefore = hash(cli);
  try {
    const client = new AislideClient(request => requestCore({ ...request, capacity_profile: 'large' }));
    const fixtures = {};
    for (const kind of ['count', 'width', 'classic']) {
      const session = await client.createPresentation(`histogram-validation-${kind}`, 'Synthetic validation fixture');
      const chart = await client.createObject({ id: 'chart', kind: 'chart', preset: kind === 'classic' ? 'column' : 'histogram' });
      if (kind === 'count') assert.deepEqual(chart.options.histogram.binning, { count: 4, rule: 'count' });
      if (kind === 'width') chart.options.histogram.binning = { rule: 'width', width: 2.5 };
      await session.transact([{ op: 'add', path: '/deck/slides/0/elements/-', value: chart }]);
      const path = join(directory, `${kind} [synthetic].pptx`);
      writeFileSync(path, Buffer.from((await session.exportPresentation()).base64, 'base64'), { flag: 'wx' });
      fixtures[kind] = path;
    }
    const parts = {};
    for (const kind of ['count', 'width']) {
      await context.test(`${kind}: strict fails and opt-in retains exactly two known diagnostics`, () => {
        const strict = validate(fixtures[kind]);
        assertSummary(strict, { compatibility: false, known: 2 });
        assert.deepEqual(strict.issues.map(issue => issue.Id).sort(), knownIds);
        const nodeName = kind === 'count' ? 'binCount' : 'binSize';
        for (const issue of strict.issues) assert.ok(issue.XPath.endsWith(`cx:${nodeName}[1]`));
        parts[kind] = strict.issues[0].Part;
        const compatible = validate(fixtures[kind], true);
        assertSummary(compatible, { compatibility: true, known: 2 });
        assert.deepEqual(compatible.issues.map(({ Severity, ...issue }) => issue), strict.issues.map(({ Severity, ...issue }) => issue));
        assert.match(compatible.stdout, /accepted WITH known schema deviations/);
      });
    }
    await context.test('classic charts remain schema-valid in both modes', () => {
      for (const compatibility of [false, true]) assertSummary(validate(fixtures.classic, compatibility), { compatibility });
    });
    await context.test('explicit newer targets apply equally to both modes', () => {
      for (const compatibility of [false, true]) {
        assertSummary(validate(fixtures.count, compatibility, 'Office2021'), { compatibility, known: 2, targetVersion: 'Office2021' });
      }
    });
    await context.test('targets that would hide chartEx errors are rejected', () => {
      const result = validate(fixtures.count, true, 'Office2007');
      assert.notEqual(result.status, 0);
      assert.equal(result.summary, undefined);
    });
    const malformed = [
      ...['0', '-1', '129', '1.5', 'NaN', 'INF', '2147483648'].map(value => ({ name: `count-${value}`, kind: 'count', mutation: 'value', value })),
      ...['0', '-1', 'NaN', 'INF', '-INF', 'Infinity', '1e16'].map(value => ({ name: `width-${value}`, kind: 'width', mutation: 'value', value })),
      ...['extra-attribute', 'qualified-val', 'mixed-text', 'whitespace-text', 'nested-child', 'duplicate-value', 'both-values',
        'wrong-layout', 'missing-layout', 'wrong-parent', 'wrong-ancestor', 'wrong-namespace'].map(mutation => ({ name: mutation, kind: 'count', mutation })),
    ];
    for (const entry of malformed) {
      await context.test(`opt-in rejects ${entry.name} without classifying the node as known`, () => {
        const path = mutate(fixtures[entry.kind], join(directory, `${entry.name}.pptx`), parts[entry.kind], entry.mutation, entry.value);
        const result = validate(path, true);
        assert.ok(result.issues.length > 0, result.stdout + result.stderr);
        assertSummary(result, { compatibility: true, unexpected: result.issues.length });
      });
    }
    for (const mutation of ['invalid-interval', 'unrelated-error']) {
      await context.test(`known child pair does not conceal ${mutation}`, () => {
        const path = mutate(fixtures.count, join(directory, `${mutation}.pptx`), parts.count, mutation);
        const result = validate(path, true);
        assert.ok(result.issues.length > 2);
        assertSummary(result, { compatibility: true, known: 2, unexpected: result.issues.length - 2 });
      });
    }
    for (const [kind, value] of [['count', '1'], ['count', '128'], ['width', '1e15'], ['width', '0.125']]) {
      await context.test(`${kind} accepts the bounded value ${value}`, () => {
        const path = mutate(fixtures[kind], join(directory, `bounded-${kind}-${value}.pptx`), parts[kind], 'value', value);
        assertSummary(validate(path, true), { compatibility: true, known: 2 });
      });
    }
    await context.test('schema text encoding remains valid without a compatibility exception', () => {
      const path = mutate(fixtures.count, join(directory, 'text-encoding.pptx'), parts.count, 'text-encoding');
      for (const compatibility of [false, true]) assertSummary(validate(path, compatibility), { compatibility });
    });
    for (const series of [128, 129]) {
      await context.test(`${series} histogram series reaching the 256-issue limit fail closed`, () => {
        const path = mutate(fixtures.count, join(directory, `cutoff-${series}.pptx`), parts.count, 'many-series', String(series));
        const result = validate(path, true);
        assertSummary(result, { compatibility: true, known: 256, limited: true });
        assert.match(result.stderr, /issue limit reached/);
      });
    }
  } finally {
    assert.equal(hash(cli), cliBefore, 'tests must not rebuild or alter the CLI');
    rmSync(directory, { recursive: true, force: true });
    context.diagnostic(JSON.stringify({ cliSha256: cliBefore, sourceBytesUnchanged: true, temporaryDirectoryRemoved: directory, validations: evidence }));
  }
});