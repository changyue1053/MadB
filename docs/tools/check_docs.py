#!/usr/bin/env python3
"""Static documentation checks; no browser, SDK execution, network or product certification."""
from __future__ import annotations
import argparse
from contextlib import redirect_stdout
import hashlib
from html.parser import HTMLParser
import io
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import zipfile
from urllib.parse import unquote, urlsplit
import render_visuals

ROOT = Path(__file__).resolve().parents[1]

class Page(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids, self.links, self.scripts = [], [], []
        self.active = None
        self.body = ''
    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if 'id' in a: self.ids.append(a['id'])
        if tag == 'a' and 'href' in a: self.links.append(a['href'])
        if tag == 'script': self.active, self.body = a, ''
    def handle_endtag(self, tag):
        if tag == 'script' and self.active is not None:
            self.scripts.append((self.active, self.body))
            self.active = None
    def handle_data(self, data):
        if self.active is not None: self.body += data

def unique_object(items):
    result = {}
    for key, value in items:
        if key in result: raise ValueError(f'Duplicate JSON key: {key}')
        result[key] = value
    return result

def load(path):
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=unique_object)

def columns(line):
    line = re.sub(r'`[^`]*`', 'CODE', line.strip())
    return len(re.split(r'(?<!\\)\|', line.strip('|')))

SOURCE_ROW = re.compile(r'^\|\s*\*\*(\d+)\.\s*(.+?)\*\*\s*\|')

def source_coverage_errors(coverage, resolve):
    """Check revised locations and their immutable inputs; never extract or execute a snapshot."""
    errors, sources, seen = [], {}, set()
    def require(ok, message):
        if not ok: errors.append(message)
    for source in coverage['sources']:
        pair = source['pair']
        require(pair not in sources, f'Duplicate historical source: {pair}')
        path = resolve(source['path'])
        if path is None or not path.is_file():
            errors.append(f'Historical source missing: {pair}')
            sources[pair] = (source, [])
            continue
        data = path.read_bytes()
        require(hashlib.sha256(data).hexdigest() == source['sha256'], f'Historical source changed: {pair}')
        lines = data.decode('utf-8').splitlines()
        numbers = [int(m[1]) for line in lines if (m := SOURCE_ROW.match(line))]
        require(numbers == list(range(1, source['items'] + 1)), f'Historical item sequence: {pair}')
        sources[pair] = (source, lines)
    for item in coverage['mappings']:
        identity = item['source_id']
        require(identity not in seen, f'Duplicate historical mapping: {identity}')
        seen.add(identity)
        pair, _, number = identity.rpartition(':')
        if pair not in sources or not number.isdigit():
            errors.append(f'Unknown historical mapping source: {identity}')
            continue
        source, lines = sources[pair]
        require(item['path'] == source['path'], f'Historical mapping path: {identity}')
        line = item['line']
        match = SOURCE_ROW.match(lines[line - 1]) if type(line) is int and 0 < line <= len(lines) else None
        require(match is not None and int(match[1]) == int(number) and match[2] == item['source_title'],
                f'Historical mapping location/title: {identity}')
    expected = {f'{pair}:{n}' for pair, (source, _) in sources.items() for n in range(1, source['items'] + 1)}
    require(seen == expected, 'Historical mappings do not cover source items exactly')

    snapshot = coverage.get('archive_snapshot')
    if snapshot:
        path = resolve(snapshot['path'])
        if path is None or not path.is_file():
            errors.append('Historical snapshot missing')
            return errors
        require(hashlib.sha256(path.read_bytes()).hexdigest() == snapshot['sha256'], 'Historical snapshot digest mismatch')
        try:
            with zipfile.ZipFile(path) as archive:
                names = archive.namelist()
                require(len(names) == len(set(names)), 'Historical snapshot duplicate members')
                manifest = json.loads(archive.read(snapshot['manifest']), object_pairs_hook=unique_object)
                require(set(names) == set(manifest['files']) | {snapshot['manifest']}, 'Historical snapshot member manifest mismatch')
                for name, digest in manifest['files'].items():
                    require(hashlib.sha256(archive.read(name)).hexdigest() == digest, f'Historical snapshot member changed: {name}')
                original = json.loads(archive.read(snapshot['coverage']), object_pairs_hook=unique_object)
                for source in original['sources']:
                    member = 'sources/' + '/'.join(Path(source['path']).parts[-2:])
                    require(hashlib.sha256(archive.read(member)).hexdigest() == source['sha256'],
                            f'Historical original source digest: {source["pair"]}')
                def identities(value):
                    return {item['source_id']: (item['source_title'], item['problem_ids'], item['classification'], item['evidence'])
                            for item in value['mappings']}
                require(identities(original) == identities(coverage), 'Historical problem identity or mapping changed')
                require({s['pair']:s['items'] for s in original['sources']} == {s['pair']:s['items'] for s in coverage['sources']},
                        'Historical source inventory changed')
        except (OSError, ValueError, KeyError, UnicodeError, zipfile.BadZipFile) as exc:
            errors.append(f'Historical snapshot invalid: {exc}')
    return errors

def validate(node, reference_root=None):
    errors, counts = [], dict(markdown=0, json=0, toml=0, tables=0, local_links=0, html=0, scripts=0)
    def check(ok, message):
        if not ok: errors.append(message)
    def local_target(path, href):
        url = urlsplit(href)
        if url.scheme or url.netloc: return None
        target = (path.parent / unquote(url.path)).resolve() if url.path else path
        if not target.is_relative_to(ROOT) and reference_root:
            target = (reference_root / path.relative_to(ROOT)).parent / unquote(url.path)
            target = target.resolve()
        return target
    def link(path, href):
        target = local_target(path, href)
        if target is None: return
        counts['local_links'] += 1
        check(target.is_file(), f'Broken link: {path.relative_to(ROOT)} → {href}')
        fragment = unquote(urlsplit(href).fragment)
        if fragment and target.suffix == '.html' and target.is_file():
            page = pages.get(target)
            if page is None:
                page = Page(); page.feed(target.read_text()); pages[target] = page
            dynamic = fragment in dynamic_ids.get(target.stem, set())
            check(fragment in page.ids or dynamic, f'Unknown HTML anchor: {href}')
    plan = load(ROOT/'catalog/implementation-plan.json')
    routes = load(ROOT/'catalog/routes.json')
    problems = load(ROOT/'catalog/problems.json')
    groups = load(ROOT/'catalog/case-groups.json')
    coverage = load(ROOT/'catalog/source-coverage.json')
    task_ids = {t['id'] for t in plan['tasks']}
    problem_ids = {p['id'] for p in problems['problems']}
    route_ids = {r['id'] for r in routes['routes']}
    group_ids = {g['id'] for g in groups['groups']}
    pages, dynamic_ids = {}, {'implementation-progress': {'task-'+id for id in task_ids}}
    for f in (ROOT/'catalog/route-views').glob('*.json'):
        v = load(f); dynamic_ids[v['slug']] = {p['id'] for p in v['profiles']}
    for path in sorted(ROOT.rglob('*')):
        if not path.is_file() or '__pycache__' in path.parts: continue
        if path.suffix == '.json':
            counts['json'] += 1
            try: load(path)
            except (ValueError, OSError) as exc: errors.append(f'{path.relative_to(ROOT)}: {exc}')
        elif path.suffix == '.toml':
            counts['toml'] += 1
            try: tomllib.loads(path.read_text())
            except ValueError as exc: errors.append(f'{path.relative_to(ROOT)}: {exc}')
        elif path.suffix == '.md':
            counts['markdown'] += 1
            fence, lines = None, []
            for line in path.read_text().splitlines():
                match = re.match(r'^\s*(`{3,}|~{3,})', line)
                if match:
                    mark = match.group(1)
                    if fence is None: fence = mark
                    elif mark[0] == fence[0] and len(mark) >= len(fence): fence = None
                elif fence is None: lines.append(line)
            check(fence is None, f'Unclosed fence: {path.relative_to(ROOT)}')
            for match in re.finditer(r'(?<!!)\[[^\]\n]+\]\((<[^>]+>|[^\s)]+)(?:\s+"[^"]*")?\)', '\n'.join(lines)):
                link(path, match.group(1).strip('<>'))
            for i, line in enumerate(lines):
                if i and re.match(r'^\s*\|?\s*:?-{3,}:?\s*\|', line):
                    counts['tables'] += 1
                    width, j = columns(lines[i-1]), i+1
                    while j < len(lines) and lines[j].strip().startswith('|'):
                        check(columns(lines[j]) == width, f'Table width: {path.relative_to(ROOT)}: {lines[j][:60]}')
                        j += 1
    for name in ['routes','architecture-map','problems','case-groups','source-coverage']:
        check(load(ROOT/f'catalog/{name}.json')['baseline'] == plan['baseline'], f'Baseline: {name}')
    for items in [problems['problems'], groups['groups']]:
        check(len({i['id'] for i in items}) == len(items), 'Duplicate problem/group ID')
    for problem in problems['problems']:
        check(set(problem['required_case_groups']) <= group_ids, f"Missing groups: {problem['id']}")
    for group in groups['groups']:
        check(set(group['problem_ids']) <= problem_ids, f"Unknown group problem: {group['id']}")
        for scenario in group.get('scenario_refs', []):
            target = (ROOT/scenario['document']).resolve()
            valid = target.is_relative_to(ROOT) and target.is_file()
            check(valid, f"Missing scenario specification: {group['id']}")
            if valid:
                check(bool(re.search(r'^## '+re.escape(scenario['id'])+r'\b', target.read_text(), re.M)),
                      f"Unknown critical scenario: {group['id']} → {scenario['id']}")
    for item in coverage['mappings']:
        check(set(item['problem_ids']) <= problem_ids, f"Unknown historical mapping: {item['source_id']}")
    errors.extend(source_coverage_errors(coverage, lambda href: local_target(ROOT/'catalog/source-coverage.json', href)))
    source_checks = [source['pair'] for source in coverage['sources']]
    audit_sources = {s['id'] for s in coverage.get('audit_sources', [])}
    audit_ids = {a['audit_id'] for a in coverage.get('audit_mappings', [])}
    for audit in coverage.get('audit_mappings', []):
        for field, allowed in [('source_ids',audit_sources),('task_ids',task_ids),('problem_ids',problem_ids),('route_ids',route_ids)]:
            check(set(audit[field]) <= allowed, f"Audit reference: {audit['audit_id']} {field}")
    for group in groups['groups']:
        for scenario in group.get('platform_scenarios', []):
            check(scenario['audit_id'] in audit_ids and set(scenario['route_ids']) <= route_ids, f"Platform scenario: {group['id']}")
    # Schemas validate structure only; identity checks do not execute a protocol session.
    schema_counts = {}
    try:
        from jsonschema import Draft202012Validator
        validators = {}
        for key, name in [('manifest','extension-manifest'),('frame','extension-wire')]:
            schema = load(ROOT/f'schemas/{name}.schema.json')
            Draft202012Validator.check_schema(schema); validators[key] = Draft202012Validator(schema)
        manifest = load(ROOT/'examples/extensions/dart-ffi.extension.example.json')
        check(validators['manifest'].is_valid(manifest), 'Manifest example')
        session = load(ROOT/'examples/extensions/protocol-session.example.json')
        frames = session['frames'] if isinstance(session, dict) else session
        digest = 'sha256:'+hashlib.sha256((ROOT/'examples/extensions/dart-ffi.extension.example.json').read_bytes()).hexdigest()
        for frame in frames:
            check(validators['frame'].is_valid(frame), 'Invalid protocol frame')
            if 'manifest_digest' in frame: check(frame['manifest_digest'] == digest, 'Manifest transcript digest')
        vectors = load(ROOT/'examples/extensions/schema-vectors.json')['vectors']
        for vector in vectors:
            check(validators[vector['subject']].is_valid(vector['document']) == (vector['expected_schema']=='valid'), f"Schema vector: {vector['id']}")
        schema_counts = dict(schemas=2, frames=len(frames), vectors=len(vectors))
    except ImportError:
        errors.append('Missing jsonschema: install a Python package exposing Draft202012Validator; schema checks not executed')
    if not node: errors.append('Node.js required for inline script syntax checks; pass --node')
    with tempfile.TemporaryDirectory(prefix='madb-doc-check-') as tmp:
        for path in sorted((ROOT/'visuals').glob('*.html')):
            counts['html'] += 1
            page = Page(); page.feed(path.read_text()); pages[path] = page
            check(len(page.ids) == len(set(page.ids)), f'Duplicate HTML IDs: {path.name}')
            for href in page.links: link(path, href)
            for index, (attrs, script) in enumerate(page.scripts):
                if attrs.get('type') == 'application/json': json.loads(script, object_pairs_hook=unique_object)
                elif not attrs.get('src') and node:
                    js = Path(tmp)/f'{path.stem}-{index}.js'; js.write_text(script)
                    result = subprocess.run([node,'--check',str(js)], capture_output=True, text=True)
                    check(result.returncode == 0, f'JS syntax {path.name}: {result.stderr}')
                    counts['scripts'] += 1
    try:
        with redirect_stdout(io.StringIO()): render_visuals.render(check=True)
    except (AssertionError, ValueError, KeyError) as exc: errors.append('Renderer: '+str(exc))
    return dict(schema='madb.documentation-validation/2', baseline=plan['baseline'], status='passed' if not errors else 'failed',
                scope='static-documents-and-rendered-html', counts=counts, extension_structure=schema_counts,
                catalog=dict(tasks=len(task_ids), routes=len(route_ids), problems=len(problem_ids), case_groups=len(group_ids), historical_sources=len(source_checks), historical_mappings=len(coverage['mappings'])),
                browser={'status':'not_executed','scope':'layout, real interactions and console'},
                product_tests_executed=False, errors=errors)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', default=shutil.which('node'), help='Node executable, used only for syntax checks')
    parser.add_argument('--reference-root', type=Path, help='Original docs root when checking a staging copy; only resolves historical links outside docs')
    parser.add_argument('--report', type=Path, help='Explicit output; default prints report only')
    args = parser.parse_args()
    result = validate(args.node, args.reference_root)
    text = json.dumps(result,ensure_ascii=False,indent=2)+'\n'
    if args.report: args.report.write_text(text)
    print(text, end='')
    return bool(result['errors'])

if __name__ == '__main__': sys.exit(main())
