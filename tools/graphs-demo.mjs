import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Users, Server, Database, ShieldCheck, Cloud, Router } from 'lucide-react';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { unzipSync } from 'fflate';

const withIcons = process.argv.includes('--icons');
const withCloudIcons = process.argv.includes('--cloud-icons');
assert.ok(!(withIcons && withCloudIcons), '--icons and --cloud-icons are mutually exclusive');
const destinations = process.argv.slice(2).filter((argument) => !['--icons', '--cloud-icons'].includes(argument));
assert.ok(destinations.length <= 1 && destinations.every((argument) => !argument.startsWith('--')), 'Usage: node tools/graphs-demo.mjs [new-output-directory] [--icons | --cloud-icons]');
const directory = resolve(destinations[0] ?? `.artifacts/graphs${withCloudIcons ? '-cloud-icons' : withIcons ? '-icons' : ''}-${Date.now()}`);
await mkdir(directory, { recursive: true });
assert.equal((await readdir(directory)).length, 0, 'Use a new or empty output directory');
const environment = {};
if (process.env.AISLIDE_CORE_BINARY !== undefined) environment.AISLIDE_CORE_BINARY = process.env.AISLIDE_CORE_BINARY;
if (withCloudIcons && process.env.AISLIDE_ICON_PACK_ROOT !== undefined) environment.AISLIDE_ICON_PACK_ROOT = process.env.AISLIDE_ICON_PACK_ROOT;
const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe', env: environment });
const client = new Client({ name: 'architecture-qualification', version: '1.0.0' });
const calls = [];
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const call = async (name, args = {}) => {
  const started = performance.now();
  let progressCount = 0;
  const result = await client.callTool({ name, arguments: args }, undefined, { timeout: 320000, maxTotalTimeout: 320000, resetTimeoutOnProgress: true, onprogress: () => { progressCount += 1; } });
  assert.ok(!result.isError, JSON.stringify(result.content));
  calls.push({ name, milliseconds: Math.round(performance.now() - started), progress_notifications: progressCount });
  return JSON.parse(result.content[0].text);
};

async function cloudExamples() {
  const catalog = await call('architecture_icons');
  assert.ok(catalog.configured, catalog.message ?? 'Local cloud icon consent is required; this demo never installs or downloads artwork');
  const identities = {
    awsCloud: 'aws/group/aws-cloud', awsRegion: 'aws/group/region', awsVpc: 'aws/group/virtual-private-cloud-vpc',
    awsPublic: 'aws/group/public-subnet', awsPrivate: 'aws/group/private-subnet', ec2: 'aws/service/amazon-ec2',
    nat: 'aws/resource/amazon-vpc-nat-gateway', gateway: 'aws/resource/amazon-vpc-internet-gateway',
    alb: 'aws/resource/elastic-load-balancing-application-load-balancer',
    vnet: 'azure/service-resource/10061-icon-service-virtual-networks', subnet: 'azure/service-resource/02742-icon-service-subnet',
    endpoint: 'azure/service-resource/02579-icon-service-private-endpoints', apps: 'azure/service-resource/10035-icon-service-app-services',
    sql: 'azure/service-resource/10130-icon-service-sql-database', vault: 'azure/service-resource/10245-icon-service-key-vaults',
    storage: 'azure/service-resource/10086-icon-service-storage-accounts', monitor: 'azure/service-resource/00001-icon-service-monitor',
    entra: 'azure/entra/microsoft-entra-id', waf: 'azure/service-resource/10362-icon-service-web-application-firewall-policies-waf',
    networking: 'gcp/category/networking', identity: 'gcp/category/security-identity', storageCategory: 'gcp/category/storage',
    run: 'gcp/core-product/cloud-run', gke: 'gcp/core-product/gke', compute: 'gcp/core-product/compute-engine',
    cloudSql: 'gcp/core-product/cloud-sql', cloudStorage: 'gcp/core-product/cloud-storage',
  };
  const metadata = new Map(catalog.icons.map((icon) => [icon.id, icon]));
  const ids = [...new Set(Object.values(identities))];
  assert.ok(ids.length <= 60);
  for (const id of ids) assert.ok(metadata.has(id), `Missing catalog identity: ${id}`);
  const result = await call('architecture_icon_assets', { ids });
  assert.deepEqual(result.icons.map((icon) => icon.id).sort(), [...ids].sort());
  let rawBytes = 0;
  const assets = new Map(result.icons.map((asset) => {
    const entry = metadata.get(asset.id);
    const bytes = Buffer.from(asset.base64, 'base64');
    assert.equal(asset.mime_type, 'image/png');
    assert.equal(asset.alt, entry.name);
    assert.equal(bytes.length, entry.png_bytes);
    assert.equal(sha256(bytes), entry.png_sha256);
    assert.deepEqual([asset.width, asset.height], [entry.width, entry.height]);
    assert.deepEqual([bytes.readUInt32BE(16), bytes.readUInt32BE(20)], [entry.width, entry.height]);
    rawBytes += bytes.length;
    return [asset.id, { base64: asset.base64, mime_type: asset.mime_type, alt: asset.alt }];
  }));
  assert.ok(rawBytes <= 4 * 1024 * 1024);
  const svg = renderToStaticMarkup(createElement(Users, { size: 24, color: '#263238', strokeWidth: 2 }));
  const users = await call('create_graph_icon', { base64: Buffer.from(svg).toString('base64'), mime_type: 'image/svg+xml', alt: 'External clients (Lucide Users)' });
  const icon = (role) => role === 'users' ? users : assets.get(identities[role]);
  const node = (id, role, label, x, y, width = 176, height = 128, group) => ({ id, label, x, y, width, height, font_size: 12, kind: 'rectangle', presentation: 'icon', icon: icon(role), ...(group ? { group } : {}) });
  const group = (id, role, label, x, y, width, height, parent, fill = 'F6F8FA') => ({ id, label, x, y, width, height, fill, stroke: '87949D', ...(parent ? { parent } : {}), ...(role ? { icon: icon(role) } : {}) });
  const edge = (id, source, target, source_port = 'right', target_port = 'left', label = '') => ({ id, source, target, source_port, target_port, route: 'elbow', label });
  const subtitle = 'Synthetic example / not a deployed system';
  const specs = [
    {
      version: 1, title: 'AWS / Two-region application paths', subtitle,
      groups: [
        group('cloud', 'awsCloud', 'AWS Cloud', 16, 96, 1120, 408),
        group('west', 'awsRegion', 'AWS Region / West', 32, 144, 472, 352, 'cloud', 'F0F6FC'),
        group('east', 'awsRegion', 'AWS Region / East', 648, 144, 472, 352, 'cloud', 'F0F6FC'),
        group('west-vpc', 'awsVpc', 'Amazon VPC / West', 48, 192, 440, 296, 'west'),
        group('east-vpc', 'awsVpc', 'Amazon VPC / East', 664, 192, 440, 296, 'east'),
        group('west-public', 'awsPublic', 'Public subnet', 64, 248, 204, 232, 'west-vpc', 'F0F8F3'),
        group('west-private', 'awsPrivate', 'Private subnet', 276, 248, 196, 232, 'west-vpc', 'FFFFFF'),
        group('east-public', 'awsPublic', 'Public subnet', 680, 248, 204, 232, 'east-vpc', 'F0F8F3'),
        group('east-private', 'awsPrivate', 'Private subnet', 892, 248, 196, 232, 'east-vpc', 'FFFFFF'),
      ],
      nodes: [
        node('user', 'users', 'External\nclients', 520, 144, 104),
        node('nat', 'nat', 'Amazon VPC\nNAT Gateway', 76, 304, 176, 128, 'west-public'),
        node('west-app', 'ec2', 'Amazon Elastic\nCompute Cloud', 284, 304, 176, 128, 'west-private'),
        node('gateway', 'gateway', 'Amazon VPC\nInternet Gateway', 516, 328, 124, 104, 'cloud'),
        node('balancer', 'alb', 'Elastic Load Balancing\nApplication Load Balancer', 692, 304, 176, 128, 'east-public'),
        node('east-app', 'ec2', 'Amazon Elastic\nCompute Cloud', 900, 304, 176, 128, 'east-private'),
      ],
      edges: [edge('request', 'user', 'gateway', 'bottom', 'top', '1 HTTPS'), edge('ingress', 'gateway', 'balancer'), edge('workload', 'balancer', 'east-app'), edge('outbound', 'west-app', 'nat', 'left', 'right'), edge('egress', 'nat', 'gateway', 'bottom', 'bottom')],
    },
    {
      version: 1, title: 'Azure / Private application access', subtitle,
      groups: [
        group('vnet', 'vnet', 'Azure Virtual Network', 16, 280, 568, 224),
        group('app-subnet', 'subnet', 'App integration', 32, 320, 248, 176, 'vnet', 'F0F6FC'),
        group('data-subnet', 'subnet', 'Private endpoints', 312, 320, 256, 176, 'vnet', 'F0F8F3'),
        group('managed', null, 'Azure managed services', 608, 280, 528, 224, undefined, 'FFFFFF'),
      ],
      nodes: [
        node('user', 'users', 'External\nclients', 16, 104, 64),
        node('waf', 'waf', 'Azure Web\nApplication Firewall', 200, 104),
        node('monitor', 'monitor', 'Azure Monitor', 752, 104),
        node('identity', 'entra', 'Microsoft Entra ID', 944, 104),
        node('app', 'apps', 'Azure App Service', 64, 360, 176, 128, 'app-subnet'),
        node('endpoint', 'endpoint', 'Azure Private Endpoint', 352, 360, 176, 128, 'data-subnet'),
        node('sql', 'sql', 'Azure SQL Database', 620, 352, 152, 128, 'managed'),
        node('vault', 'vault', 'Azure Key Vault', 796, 352, 152, 128, 'managed'),
        node('storage', 'storage', 'Azure Storage account', 972, 352, 152, 128, 'managed'),
      ],
      edges: [edge('request', 'user', 'waf', 'right', 'left', '1 HTTPS'), edge('filtered', 'waf', 'app', 'bottom', 'right'), edge('private', 'app', 'endpoint', 'right', 'left', '2 HTTPS'), edge('data', 'endpoint', 'sql', 'right', 'left', '3 SQL')],
    },
    {
      version: 1, title: 'Google Cloud / Workload and data paths', subtitle,
      groups: [
        group('vpc', 'networking', 'Google Cloud VPC', 16, 256, 744, 248),
        group('workloads', 'networking', 'Workload subnet', 32, 300, 344, 196, 'vpc', 'F0F6FC'),
        group('data', 'networking', 'Data subnet', 400, 300, 344, 196, 'vpc', 'F0F8F3'),
        group('storage', 'storageCategory', 'Google Cloud Storage', 784, 256, 352, 248, undefined, 'FFFFFF'),
        group('identity', 'identity', 'Security / identity', 784, 88, 352, 160),
      ],
      nodes: [
        node('user', 'users', 'External clients', 16, 104, 128),
        node('balancer', 'networking', 'Google Cloud\nLoad Balancing', 304, 104),
        node('run', 'run', 'Google Cloud Run', 552, 104),
        node('gke', 'gke', 'Google\nKubernetes Engine', 48, 348, 152, 128, 'workloads'),
        node('compute', 'compute', 'Google Compute Engine', 208, 348, 152, 128, 'workloads'),
        node('sql', 'cloudSql', 'Google Cloud SQL', 488, 348, 176, 128, 'data'),
        node('objects', 'cloudStorage', 'Google Cloud Storage', 944, 348, 176, 128, 'storage'),
        node('iam', 'identity', 'Google Cloud IAM', 872, 128, 176, 112, 'identity'),
      ],
      edges: [edge('request', 'user', 'balancer', 'right', 'left', '1 HTTPS'), edge('service', 'balancer', 'run'), edge('serverless-data', 'run', 'sql', 'bottom', 'top'), edge('worker', 'gke', 'compute'), edge('database', 'compute', 'sql', 'right', 'left', '3 Data')],
    },
  ];
  return { specs, selected: ids.map((id) => metadata.get(id)), raw_bytes: rawBytes, catalog_count: catalog.icons.length, providers: catalog.providers };
}

function assertAuthored(actual, expected, path = 'graph') {
  if (Array.isArray(expected)) {
    assert.equal(actual.length, expected.length, path);
    expected.forEach((value, index) => assertAuthored(actual[index], value, `${path}[${index}]`));
  } else if (expected && typeof expected === 'object') {
    for (const [key, value] of Object.entries(expected)) assertAuthored(actual[key], value, `${path}.${key}`);
  } else assert.deepEqual(actual, expected, path);
}

function inspectCloudDocument(document, specs) {
  const rect = ({ x, y, width, height }) => ({ x, y, width, height });
  const overlaps = (first, second) => first.x < second.x + second.width - 0.01 && first.x + first.width > second.x + 0.01 && first.y < second.y + second.height - 0.01 && first.y + first.height > second.y + 0.01;
  const contains = (parent, child) => child.x >= parent.x + 8 && child.y >= parent.y + 40 && child.x + child.width <= parent.x + parent.width - 8 && child.y + child.height <= parent.y + parent.height - 8;
  return specs.map((spec, index) => {
    const root = document.deck.slides[index].elements.find((element) => element.id === `graph-${index + 1}`);
    assert.equal(root.type, 'group');
    const find = (suffix) => {
      const matches = root.children.filter((element) => element.id.endsWith(suffix));
      assert.equal(matches.length, 1, `Missing or ambiguous ${suffix}`);
      return matches[0];
    };
    assert.ok(spec.groups.length <= 16);
    const groups = new Map(spec.groups.map((region) => [region.id, region]));
    let maxDepth = 0;
    for (const region of spec.groups) {
      const ancestors = new Set([region.id]);
      let parent = region.parent;
      if (parent) assert.ok(contains(groups.get(parent), region), `Group containment: ${region.id}`);
      while (parent) {
        assert.ok(!ancestors.has(parent), 'Group cycle');
        ancestors.add(parent);
        assert.ok(groups.has(parent));
        parent = groups.get(parent).parent;
      }
      maxDepth = Math.max(maxDepth, ancestors.size);
    }
    assert.ok(maxDepth <= 4);
    const entities = [...spec.nodes.map((node) => ({ entity: node, kind: 'node', imagePrefix: '-ni-', labelPrefix: '-nt-' })), ...spec.groups.map((region) => ({ entity: region, kind: 'group', imagePrefix: '-gi-', labelPrefix: '-gt-' }))].map(({ entity, kind, imagePrefix, labelPrefix }) => {
      assert.ok(entity.x >= 0 && entity.y >= 88 && entity.x + entity.width <= 1152 && entity.y + entity.height <= 512);
      if (entity.group) assert.ok(contains(groups.get(entity.group), entity), `Node containment: ${entity.id}`);
      const anchor = find((kind === 'node' ? '-n-' : '-g-') + entity.id);
      assert.ok(['shape', 'text', 'rect'].includes(anchor.type));
      assert.equal(anchor.text ?? '', '');
      for (const key of ['x', 'y', 'width', 'height']) assert.ok(Math.abs(anchor[key] - entity[key]) < 0.001, `Native bounds: ${entity.id}.${key}`);
      const label = find(labelPrefix + entity.id);
      assert.equal(label.text, entity.label);
      const detail = { id: entity.id, native_id: anchor.id, native_type: anchor.type, kind, parent: entity.parent ?? entity.group ?? null, bounds: rect(anchor), label: entity.label, label_bounds: rect(label), label_font_size: label.font_size };
      if (entity.icon) {
        assert.deepEqual(Object.keys(entity.icon).sort(), ['alt', 'base64', 'mime_type']);
        const picture = find(imagePrefix + entity.id);
        assert.equal(picture.type, 'picture');
        assert.equal(picture.base64, entity.icon.base64);
        assert.equal(picture.mime_type, entity.icon.mime_type);
        assert.equal(picture.alt, entity.icon.alt);
        assert.ok(!overlaps(picture, label), `Picture/label overlap: ${entity.id}`);
        Object.assign(detail, { picture_id: picture.id, picture_bounds: rect(picture), alt: picture.alt, png_sha256: sha256(Buffer.from(picture.base64, 'base64')) });
      }
      return detail;
    });
    assert.equal(root.children.filter((element) => element.type === 'picture').length, entities.filter((entity) => entity.picture_id).length);
    const connections = spec.edges.map((edge) => {
      const connector = find('-e-' + edge.id);
      assert.equal(connector.type, 'connector');
      assert.equal(connector.start.element_id, find('-n-' + edge.source).id);
      assert.equal(connector.end.element_id, find('-n-' + edge.target).id);
      for (const endpoint of [connector.start, connector.end]) assert.ok(Number.isInteger(endpoint.site));
      return { id: edge.id, source: edge.source, target: edge.target, start: connector.start, end: connector.end, bounds: rect(connector), routing: connector.routing, ...(edge.label ? { label: edge.label, label_bounds: rect(find('-et-' + edge.id)) } : {}) };
    });
    return { slide_id: document.deck.slides[index].id, graph_id: root.id, title: spec.title, root_bounds: rect(root), view_width: root.view_width, view_height: root.view_height, max_depth: maxDepth, entities, connections };
  });
}

function verifyMedia(bytes, specs) {
  const entries = unzipSync(bytes);
  const media = Object.entries(entries).filter(([name]) => name.startsWith('ppt/media/')).map(([name, data]) => ({ name, bytes: data.length, sha256: sha256(data) }));
  const expected = new Set(specs.flatMap((spec) => [...spec.nodes, ...spec.groups].filter((entity) => entity.icon).map((entity) => sha256(Buffer.from(entity.icon.base64, 'base64')))));
  assert.deepEqual(new Set(media.map((entry) => entry.sha256)), expected);
  return media;
}

try {
  await client.connect(transport);
  const catalog = await call('graph_catalog');
  let cloud;
  const specs = catalog.examples.map((entry) => structuredClone(entry.spec));
  specs.push({
    version: 1, title: 'Routed exchange', subtitle: 'Synthetic example / attached, dashed and bidirectional connectors',
    nodes: [{ id: 'client', label: 'Client', x: 48, y: 128, width: 240, height: 104 }, { id: 'service', label: 'Service', kind: 'rounded_rectangle', x: 560, y: 320, width: 240, height: 104 }],
    edges: [{ id: 'reply', source: 'service', target: 'client', source_port: 'left', target_port: 'right', route: 'elbow', start_arrow: true, dashed: true, label: 'Response' }], groups: [],
  });
  specs.push({
    version: 1, title: 'Editable graph shapes', subtitle: 'Synthetic example / each label and connector remains native',
    nodes: catalog.shapes.map((kind, index) => ({ id: `node-${index}`, label: kind.replaceAll('_', ' '), kind, x: 40 + index % 3 * 376, y: 128 + Math.floor(index / 3) * 208, width: 264, height: 136 })),
    edges: [{ id: 'first', source: 'node-0', target: 'node-1', route: 'straight' }, { id: 'second', source: 'node-1', target: 'node-2', route: 'straight' }, { id: 'third', source: 'node-3', target: 'node-4', route: 'straight' }, { id: 'fourth', source: 'node-4', target: 'node-5', route: 'straight' }, { id: 'vertical', source: 'node-2', target: 'node-3', source_port: 'bottom', target_port: 'top', route: 'elbow' }], groups: [],
  });
  if (withCloudIcons) {
    cloud = await cloudExamples();
    specs.splice(0, specs.length, ...cloud.specs);
  }
  if (withIcons) {
    const assets = [];
    for (const [index, Icon] of [Users, Server, Database, ShieldCheck, Router, Cloud].entries()) {
      const svg = renderToStaticMarkup(createElement(Icon, { size: 24, color: ['#0017c1', '#007a4d', '#b53055'][index % 3], strokeWidth: 2 }));
      const picture = await call('create_graph_icon', { base64: Buffer.from(svg).toString('base64'), mime_type: 'image/svg+xml', alt: `Synthetic graph icon ${index + 1} (Lucide)` });
      assets.push({ base64: picture.base64, mime_type: picture.mime_type, alt: picture.alt });
    }
    const { default: sharp } = await import('sharp');
    const jpeg = await sharp(Buffer.from(assets.at(-1).base64, 'base64')).resize(96).flatten({ background: '#ffffff' }).jpeg({ quality: 90 }).toBuffer();
    assets[assets.length - 1] = { base64: jpeg.toString('base64'), mime_type: 'image/jpeg', alt: 'Synthetic JPEG cloud icon (Lucide)' };
    for (const spec of specs) {
      spec.nodes = spec.nodes.map((node, index) => ({ ...node, icon: assets[index % assets.length] }));
      spec.subtitle = 'Synthetic icon example / native pictures, labels and attached connections';
    }
  }
  const created = await call('compile_report', { report: { title: withCloudIcons ? 'AISlide cloud architecture examples' : withIcons ? 'AISlide architecture graph icons' : 'AISlide architecture graphs', subtitle: '', period: '', source: 'Synthetic qualification examples, not a production topology', sections: specs.map((spec) => ({ title: spec.title, layout: 'statement', body: [], rows: [], metrics: [] })) } });
  let document = await call('get_document', { deck_id: created.deck_id });
  await call('apply_transaction', { deck_id: created.deck_id, expected_revision: document.revision, operations: document.deck.slides.map((_, index) => ({ op: 'replace', path: `/deck/slides/${index}/elements`, value: [] })) });
  for (const [index, spec] of specs.entries()) {
    document = await call('get_document', { deck_id: created.deck_id });
    await call('add_graph', { deck_id: created.deck_id, expected_revision: document.revision, slide_id: `slide-${index + 1}`, id: `graph-${index + 1}`, spec });
  }
  document = await call('get_document', { deck_id: created.deck_id });
  const counts = {};
  function visit(elements) { for (const element of elements) { counts[element.type] = (counts[element.type] ?? 0) + 1; if (element.type === 'group') visit(element.children); } }
  for (const slide of document.deck.slides) visit(slide.elements);
  const cloudGeometry = withCloudIcons ? inspectCloudDocument(document, specs) : undefined;
  await call('export_pptx', { deck_id: created.deck_id, filename: 'architecture-graphs.pptx' });
  const original = await readFile(join(directory, 'architecture-graphs.pptx'));
  assert.equal(original.subarray(0, 2).toString(), 'PK');
  const reopened = await call('open_pptx', { base64: original.toString('base64') });
  for (let index = 0; index < specs.length; index++) {
    const graph = await call('get_graph', { deck_id: reopened.deck_id, slide_id: `slide-${index + 1}`, id: `graph-${index + 1}` });
    assert.equal(graph.stale, false);
    assert.equal(graph.spec.nodes.length, specs[index].nodes.length);
    if (withIcons) assert.deepEqual(graph.spec.nodes.map((node) => node.icon), specs[index].nodes.map((node) => node.icon));
    if (withCloudIcons) assertAuthored(graph.spec, specs[index]);
  }
  const reopenedGeometry = withCloudIcons ? inspectCloudDocument(await call('get_document', { deck_id: reopened.deck_id }), specs) : undefined;
  const operations = [{ op: 'move', ids: [specs[0].nodes[0].id], dx: 24, dy: 16 }];
  if (withIcons) operations.push({ op: 'put_node', node: { ...specs[0].nodes[0], x: specs[0].nodes[0].x + 24, y: specs[0].nodes[0].y + 16, icon: specs[0].nodes[1].icon } });
  const editStarted = performance.now();
  await call('apply_graph', { deck_id: reopened.deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'graph-1', operations });
  const editMilliseconds = Math.round(performance.now() - editStarted);
  const moved = await call('get_graph', { deck_id: reopened.deck_id, slide_id: 'slide-1', id: 'graph-1' });
  assert.equal(moved.spec.nodes[0].x, specs[0].nodes[0].x + 24);
  assert.equal(moved.spec.nodes[0].y, specs[0].nodes[0].y + 16);
  assert.equal(moved.stale, false);
  if (withIcons) assert.deepEqual(moved.spec.nodes[0].icon, specs[0].nodes[1].icon);
  await call('export_pptx', { deck_id: reopened.deck_id, filename: 'architecture-graphs-edited.pptx' });
  const edited = await readFile(join(directory, 'architecture-graphs-edited.pptx'));
  const editedOpened = await call('open_pptx', { base64: edited.toString('base64') });
  assert.equal((await call('get_graph', { deck_id: editedOpened.deck_id, slide_id: 'slide-1', id: 'graph-1' })).stale, false);
  let editedGeometry;
  if (withCloudIcons) {
    const editedSpecs = structuredClone(specs);
    editedSpecs[0].nodes[0].x += 24;
    editedSpecs[0].nodes[0].y += 16;
    for (const [index, spec] of editedSpecs.entries()) {
      const graph = await call('get_graph', { deck_id: editedOpened.deck_id, slide_id: `slide-${index + 1}`, id: `graph-${index + 1}` });
      assert.equal(graph.stale, false);
      assertAuthored(graph.spec, spec);
    }
    editedGeometry = inspectCloudDocument(await call('get_document', { deck_id: editedOpened.deck_id }), editedSpecs);
    verifyMedia(edited, editedSpecs);
  }
  await call('undo', { deck_id: reopened.deck_id });
  await call('export_pptx', { deck_id: reopened.deck_id, filename: 'architecture-graphs-undone.pptx' });
  assert.deepEqual(await readFile(join(directory, 'architecture-graphs-undone.pptx')), original);
  assert.deepEqual(await readFile(join(directory, 'architecture-graphs.pptx')), original);
  let cloudEvidence;
  if (withCloudIcons) {
    for (const [index, spec] of specs.entries()) {
      const graph = await call('get_graph', { deck_id: reopened.deck_id, slide_id: `slide-${index + 1}`, id: `graph-${index + 1}` });
      assert.equal(graph.stale, false);
      assertAuthored(graph.spec, spec);
    }
    const previews = await call('export_static', { deck_id: reopened.deck_id, filename: 'cloud-preview.png', options: { format: 'png', scale: 1 } });
    const { default: sharp } = await import('sharp');
    const pixels = [];
    for (const file of previews.files) {
      const bytes = await readFile(join(directory, file.filename));
      assert.equal(sha256(bytes), file.sha256);
      const { data, info } = await sharp(bytes).removeAlpha().raw().toBuffer({ resolveWithObject: true });
      assert.deepEqual([info.width, info.height], [1280, 720]);
      let colored = 0;
      for (let offset = 0; offset < data.length; offset += info.channels) if (Math.max(data[offset], data[offset + 1], data[offset + 2]) - Math.min(data[offset], data[offset + 1], data[offset + 2]) > 24) colored++;
      assert.ok(colored > 1000, 'Preview must contain nonblank colored artwork');
      pixels.push({ filename: file.filename, sha256: file.sha256, width: info.width, height: info.height, colored_pixels: colored });
    }
    assert.equal(pixels.length, 3);
    cloudEvidence = {
      catalog_count: cloud.catalog_count, selected_icons: cloud.selected, requested_raw_bytes: cloud.raw_bytes,
      providers: cloud.providers, media: verifyMedia(original, specs), source_geometry: cloudGeometry, reopened_geometry: reopenedGeometry, edited_geometry: editedGeometry,
      previews: pixels, render_warnings: previews.warnings, group_icon_roundtrip: true, native_picture_bytes_and_alt_exact: true,
      authored_fields_and_ancestry_preserved: true, picture_label_bounds_disjoint: true, native_connection_references: true,
      category_mappings: [{ id: 'gcp/category/networking', labels: ['Google Cloud VPC', 'Workload subnet', 'Data subnet', 'Google Cloud Load Balancing'] }, { id: 'gcp/category/security-identity', labels: ['Security / identity', 'Google Cloud IAM'] }],
      routing_obstacle_avoidance: false, browser_visual_review: false, office_visual_parity: false,
    };
  }
  const evidence = {
    generated_at: new Date().toISOString(), transport: 'Official MCP SDK / stdio / local Rust core', slides: specs.length, objects: counts,
    graphs: specs.map((spec) => ({ title: spec.title, nodes: spec.nodes.length, connections: spec.edges.length, groups: spec.groups?.length ?? 0 })),
    original_sha256: createHash('sha256').update(original).digest('hex'), edited_sha256: createHash('sha256').update(edited).digest('hex'),
    reopened_metadata_current: true, native_graph_edit: true, undo_byte_identical: true, original_unchanged: true, office_visual_parity: false,
    ...(withIcons ? { node_icons: counts.picture, icon_formats: [...new Set(specs.flatMap((spec) => spec.nodes.map((node) => node.icon.mime_type)))], native_icon_replacement: true, native_icon_edit_milliseconds: editMilliseconds, original_bytes: original.length, edited_bytes: edited.length } : {}),
    ...(withCloudIcons ? { cloud: cloudEvidence, calls, undone_sha256: sha256(original), original_bytes: original.length, edited_bytes: edited.length } : {}),
  };
  await writeFile(join(directory, 'evidence.json'), JSON.stringify(evidence, null, 2), { flag: 'wx', encoding: 'utf8' });
  console.log(JSON.stringify({ directory, ...evidence, ...(withCloudIcons ? { cloud: { catalog_count: cloud.catalog_count, selected_icons: cloud.selected.length, previews: cloudEvidence.previews }, calls: undefined } : {}) }, null, 2));
} finally { await client.close(); }