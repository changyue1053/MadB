#!/usr/bin/env python3
"""Render portable MadB HTML views from versioned catalogs. No third-party dependencies."""
from __future__ import annotations
import argparse
import hashlib
from html import escape
import json
from pathlib import Path
import re
from urllib.parse import urldefrag, urlsplit

ROOT = Path(__file__).resolve().parents[1]
TEMPLATES = Path(__file__).resolve().parent / 'templates'
ROUTE_VIEWS = ROOT / 'catalog' / 'route-views'
VIEW_SLUGS = (
    'kotlin-rust-architecture', 'swift-rust-architecture', 'arkts-rust-architecture',
    'arkts-c-cpp-architecture', 'swift-c-cpp-architecture',
    'kotlin-c-cpp-architecture', 'rust-c-cpp-architecture', 'extensions-architecture',
)
FLOW_IDS = ('build', 'call', 'callback', 'close', 'delivery')

EXISTING_ROUTE_VIEWS = (
    dict(slug='kotlin-swift-architecture', title='Kotlin/Native ↔ Swift',
         summary='Objective-C export + SKIE 主线、NativeCoroutines 可选增强与正式 SDK 分发。',
         route_ids=['KN-SWIFT-OBJC'], extension_task_ids=['X01', 'X02'], profiles=[]),
    dict(slug='kotlin-arkts-architecture', title='Kotlin/Native ↔ ArkTS',
         summary='CPF + akinterop 主线、KNOI 独立扩展、载荷和环境生命周期。',
         route_ids=['KN-ARKTS-NAPI'], extension_task_ids=[], profiles=[]),
)

def read_json(name: str):
    return json.loads((ROOT / 'catalog' / name).read_text(encoding='utf-8'))

def encoded(data) -> str:
    # JSON script elements are inert, but </script must never terminate the element.
    return json.dumps(data, ensure_ascii=False, separators=(',', ':')).replace('<', '\\u003c').replace('&', '\\u0026').replace('\u2028', '\\u2028').replace('\u2029', '\\u2029')

def validate(plan, architecture, routes):
    tasks = plan['tasks']
    by_id = {task['id']: task for task in tasks}
    assert len(by_id) == len(tasks), 'Duplicate task ID'
    phases = {p['id'] for p in plan['phases']}
    phase_order = {p['id']: p['order'] for p in plan['phases']}
    assert len(phases) == len(plan['phases']) and len(set(phase_order.values())) == len(phases), 'Phase identity/order'
    route_ids = {r['id'] for r in routes['routes']}
    canonical = {r['protocol_route_id'] for r in routes['routes']}
    assert len(canonical) == len(route_ids), 'Duplicate protocol route ID'
    visited, active = set(), set()
    def visit(task_id):
        assert task_id in by_id, f'Missing task: {task_id}'
        assert task_id not in active, f'Dependency cycle: {task_id}'
        if task_id in visited:
            return
        active.add(task_id)
        for dependency in by_id[task_id]['depends_on']:
            visit(dependency)
        active.remove(task_id)
        visited.add(task_id)
    for task in tasks:
        visit(task['id'])
        assert task['phase'] in phases and task['start_phase'] in phases
        assert phase_order[task['start_phase']] <= phase_order[task['phase']], task['id']
        assert task['status'] in {'not_started', 'in_progress', 'blocked', 'implemented'}
        assert task['verification'] in {'missing', 'running', 'failed', 'passed', 'stale'}
        if task['verification'] == 'passed':
            assert task['status'] == 'implemented' and task['evidence'], task['id']
        assert set(task['routes']) <= route_ids
        assert task['acceptance'], task['id']
        for name in task['documents']:
            assert (ROOT / name).is_file(), name
    assert {r for t in tasks for r in t['routes']} == route_ids, 'Route plan gap'
    for extension, owners in routes['extension_work_packages'].items():
        assert owners and set(owners) <= set(by_id), f'Extension plan gap: {extension}'
        assert all(by_id[owner]['scope'] == 'extension' for owner in owners), extension
    assert {r for t in tasks for r in t['requirements']} == {f'REQ-{n:02d}' for n in range(1,15)}, 'Requirement gap'
    validate_decision_gates(plan)
    node_ids = {n['id'] for n in architecture['nodes']}
    flow_ids = {f['id'] for f in architecture['flows']}
    assert len(node_ids) == len(architecture['nodes'])
    for node in architecture['nodes']:
        assert (ROOT / node['document']).is_file(), node['document']
    for edge in architecture['edges']:
        assert edge['source'] in node_ids and edge['target'] in node_ids
        assert set(edge['flows']) <= flow_ids
    for flow in architecture['flows']:
        assert set(flow['nodes']) <= node_ids

def validate_decision_gates(plan):
    """Validate design-to-plan references; this is not product acceptance."""
    gates = plan.get('decision_gates')
    assert isinstance(gates, list) and gates, 'Missing decision gates'
    task_ids = {task['id'] for task in plan['tasks']}
    seen = set()
    for gate in gates:
        identity = gate.get('id', '')
        assert re.fullmatch(r'DEC-[0-9]{2}', identity) and identity not in seen, 'Duplicate/invalid decision ID'
        seen.add(identity)
        assert all(isinstance(gate.get(key), str) and gate[key].strip() for key in ('title','before','document')), identity
        target = (ROOT/gate['document']).resolve()
        assert target.is_relative_to(ROOT) and target.is_file(), f'{identity}: missing specification'
        assert gate['task_ids'] and len(set(gate['task_ids'])) == len(gate['task_ids']) and set(gate['task_ids']) <= task_ids, f'{identity}: task mapping'
        assert gate['acceptance'] and all(isinstance(x,str) and x.strip() for x in gate['acceptance']), f'{identity}: acceptance'
        assert identity in target.read_text(encoding='utf-8'), f'{identity}: missing normative anchor'

def decision_gate_rows(plan):
    rows = []
    for gate in plan['decision_gates']:
        label = escape(gate['id']+' · '+gate['title'])
        spec = escape('../'+gate['document'], quote=True)
        links = ' · '.join('<a href="implementation-progress.html#task-'+escape(task,quote=True)+'">'+escape(task)+'</a>' for task in gate['task_ids'])
        criteria = escape('；'.join(gate['acceptance']))
        rows.append('<tr id="decision-'+escape(gate['id'],quote=True)+'"><th scope="row"><a href="'+spec+'">'+label+'</a></th><td>'+escape(gate['before'])+'</td><td>'+criteria+'<div class="small">'+links+'</div></td></tr>')
    return '\n'.join(rows)

def project_tree_html(markdown: str, identity: str) -> str:
    """Render one canonical text tree; duplicate/malformed sources are errors."""
    start, end = f'<!-- {identity}:start -->', f'<!-- {identity}:end -->'
    assert markdown.count(start) == markdown.count(end) == 1, f'{identity}: missing/duplicate tree'
    opening = markdown.index(start) + len(start)
    closing = markdown.index(end)
    assert closing > opening, f'{identity}: reversed tree markers'
    block = markdown[opening:closing]
    match = re.fullmatch(r'\s*```text\n(?P<tree>[^`]+)\n```\s*', block)
    assert match and match['tree'].strip(), f'{identity}: malformed text tree'
    return escape(match['tree'])

def validate_route_views(views, plan, routes, problems=None):
    """Validate descriptive view data, without treating it as a product IR/schema."""
    route_ids = {route['id'] for route in routes['routes']}
    task_ids = {task['id'] for task in plan['tasks']}
    problem_ids = {p['id'] for p in (problems or read_json('problems.json'))['problems']}
    slugs = [view['slug'] for view in views]
    assert set(slugs) == set(VIEW_SLUGS) and len(slugs) == len(VIEW_SLUGS), 'Missing or duplicate route view'

    def strings(value, label, minimum=1, maximum=None):
        assert isinstance(value, list), label
        assert len(value) >= minimum and (maximum is None or len(value) <= maximum), label
        assert all(isinstance(item, str) and item.strip() for item in value), label

    def text_fields(item, keys, label):
        for key in keys:
            assert isinstance(item.get(key), str) and item[key].strip(), f'{label}: {key}'

    def references(value, allowed, label):
        strings(value, label, minimum=0)
        assert len(set(value)) == len(value) and set(value) <= allowed, label

    covered = {route for view in EXISTING_ROUTE_VIEWS for route in view['route_ids']}
    for view in views:
        slug = view['slug']
        assert view.get('view_schema') == 'madb.route-detail-view/3', slug
        text_fields(view, ('slug', 'title', 'eyebrow', 'summary'), slug)
        references(view['route_ids'], route_ids, f'{slug}: routes')
        references(view.get('extension_task_ids', []), task_ids, f'{slug}: extension tasks')
        covered.update(view['route_ids'])
        profiles = view['profiles']
        assert isinstance(profiles, list) and profiles, f'{slug}: profiles'
        profile_ids = [profile['id'] for profile in profiles]
        assert len(set(profile_ids)) == len(profile_ids), f'{slug}: duplicate profile'
        profile_routes = set()
        for profile in profiles:
            label = f"{slug}/{profile['id']}"
            text_fields(profile, ('id', 'name', 'kind', 'summary'), label)
            assert re.fullmatch(r'[A-Za-z][A-Za-z0-9_-]*', profile['id']), label
            assert profile['kind'] in {'主线候选', '可选配置', '实验扩展'}, label
            references(profile['route_ids'], route_ids, f'{label}: routes')
            references(profile['task_ids'], task_ids, f'{label}: tasks')
            strings(profile['spec_refs'], f'{label}: specs')
            for reference in profile['spec_refs']:
                target = (ROOT / reference).resolve()
                assert target.is_relative_to(ROOT) and target.is_file(), f'{label}: specification {reference}'
            for step in profile['build_steps']:
                text_fields(step, ('title', 'text'), label)
            assert [s['text'] for s in profile['build_steps']] == profile['flows'][0]['steps'], f'{label}: build step drift'
            assert profile['task_ids'], f'{label}: no implementation task'
            profile_routes.update(profile['route_ids'])
            flows = profile['flows']
            assert [flow['id'] for flow in flows] == list(FLOW_IDS), f'{label}: flow order/identity'
            for flow in flows:
                text_fields(flow, ('id', 'label'), label)
                strings(flow['steps'], f'{label}: flow steps', minimum=1)
            for key in ('generated', 'handwritten', 'delivery', 'gates', 'choices'):
                strings(profile[key], f'{label}: {key}')
            risks = profile['risks']
            assert isinstance(risks, list) and risks, f'{label}: risks'
            for risk in risks:
                text_fields(risk, ('problem', 'solution', 'example'), label)
                references(risk['problem_ids'], problem_ids, f'{label}: risk problems')
                references(risk['task_ids'], set(profile['task_ids']), f'{label}: risk owners')
                assert risk['problem_ids'] and risk['task_ids'], f'{label}: untraceable risk'
            detail = profile['detail']
            assert detail['roles'] and detail['models'], label
            assert detail['family'] and detail['category'], label
            graph = detail['graph']
            graph_ids = {node['id'] for node in graph['nodes']}
            assert len(graph_ids) == len(graph['nodes']) and graph_ids, label
            for node in graph['nodes']:
                text_fields(node, ('id','title','kind','summary','kicker'), label)
                strings(node['does'], label); strings(node['constraints'], label); strings(node['lines'], label)
                assert node['phase'] in {'build', 'runtime'}, label
                assert node['x'] >= 0 and node['y'] >= 0 and node['w'] > 0 and node['h'] > 0, label
                assert node['x']+node['w'] <= graph['width'] and node['y']+node['h'] <= graph['height'], label
            for i, node in enumerate(graph['nodes']):
                for other in graph['nodes'][i+1:]:
                    assert not (max(node['x'],other['x']) < min(node['x']+node['w'],other['x']+other['w']) and max(node['y'],other['y']) < min(node['y']+node['h'],other['y']+other['h'])), f'{label}: overlapping nodes'
            for band in graph.get('subbands', []):
                text_fields(band, ('label',), label)
                assert band['x'] >= 0 and band['y'] >= 0 and band['x']+band['w'] <= graph['width'] and band['y']+band['h'] <= graph['height'], label
            for edge in graph['edges']:
                assert edge['a'] in graph_ids and edge['b'] in graph_ids, label
                assert edge['group'] in {'build','runtime','host','lifecycle','delivery'}, label
            seq = detail['sequences']
            strings(seq['actors'], label, minimum=2)
            assert [item['id'] for item in seq['scenarios']] == ['call','reverse','close'], label
            for scenario in seq['scenarios']:
                assert scenario['events'], label
                for event in scenario['events']:
                    assert 0 <= event['from_lane'] < len(seq['actors']) and 0 <= event['to_lane'] < len(seq['actors']) and event['text'], label
            for model in detail['models']:
                strings(model['path'], label, minimum=2); text_fields(model, ('title','note'), label)
            assert detail['modules'] and detail['owners'] and detail['states'], label
            text_fields(detail['example'], ('title','language','code','note','kind'), label)
            assert detail['example']['kind'] in {'protocol-pseudocode', 'compile-checked'}, label
            if detail['example']['kind'] == 'compile-checked':
                assert detail['example'].get('evidence'), f'{label}: missing compilation evidence'
            text_fields(detail['case_study'], ('title','trigger','trace','expected','oracle'), label)
        assert set(view['route_ids']) <= profile_routes, f'{slug}: profile route gap'
        sources = view['sources']
        assert isinstance(sources, list) and sources, f'{slug}: sources'
        for source in sources:
            text_fields(source, ('label',), slug)
            assert ('path' in source) != ('url' in source), f'{slug}: source location'
            if 'path' in source:
                relative, _ = urldefrag(source['path'])
                target = (ROOT / relative).resolve()
                assert target.is_relative_to(ROOT) and target.is_file(), f'{slug}: source {relative}'
            else:
                url = urlsplit(source['url'])
                assert url.scheme == 'https' and url.netloc, f'{slug}: source URL'
    assert covered == route_ids, 'Architecture view route coverage gap'

def route_view_index(views):
    return list(EXISTING_ROUTE_VIEWS) + [
        dict(slug=view['slug'], title=view['title'], summary=view['summary'],
             route_ids=view['route_ids'], extension_task_ids=view.get('extension_task_ids', []),
             profiles=[{key: profile[key] for key in ('id', 'name', 'kind', 'route_ids', 'task_ids')}
                       for profile in view['profiles']])
        for view in views
    ]

def render(check=False):
    plan = read_json('implementation-plan.json')
    architecture = read_json('architecture-map.json')
    routes = read_json('routes.json')
    validate(plan, architecture, routes)
    view_files = sorted(ROUTE_VIEWS.glob('*.json'))
    views_by_slug = {}
    for path in view_files:
        view = json.loads(path.read_text(encoding='utf-8'))
        assert path.stem == view['slug'], f'View filename mismatch: {path.name}'
        assert view['slug'] not in views_by_slug, f'Duplicate view: {path.name}'
        views_by_slug[view['slug']] = view
    assert set(views_by_slug) == set(VIEW_SLUGS), 'Missing or unexpected route-view file'
    views = [views_by_slug[slug] for slug in VIEW_SLUGS]
    validate_route_views(views, plan, routes)
    common = (TEMPLATES / 'shared.css').read_text(encoding='utf-8')
    dataset = dict(plan=plan, architecture=architecture, routes=routes,
                   view_index=route_view_index(views),
                   plan_digest=hashlib.sha256((ROOT / 'catalog/implementation-plan.json').read_bytes()).hexdigest(),
                   counts=dict(problems=len(read_json('problems.json')['problems']),
                               groups=len(read_json('case-groups.json')['groups']),
                               sources=len(read_json('source-coverage.json')['mappings'])))
    outputs = []
    def emit(name, template_name, page_dataset):
        template = (TEMPLATES / f'{template_name}.html').read_text(encoding='utf-8')
        replacements = {'/* SHARED_CSS */': common, '/* DATA_JSON */': encoded(page_dataset)}
        if template_name == 'implementation-progress':
            replacements['<!-- DECISION_GATE_ROWS -->'] = decision_gate_rows(plan)
        if template_name == 'architecture-overview':
            for marker, source, identity in (
                ('<!-- MADB_PROJECT_TREE -->', 'architecture/02-modules-and-ports.md', 'madb-project-tree'),
                ('<!-- MADB_LAB_TREE -->', 'validation/02-lab-architecture.md', 'madb-lab-tree'),
            ):
                replacements[marker] = project_tree_html((ROOT/source).read_text(encoding='utf-8'), identity)
        if template_name == 'route-panorama':
            replacements.update({
                '/* ROUTE_CSS */': (TEMPLATES / 'route-panorama.css').read_text(encoding='utf-8'),
                '/* ROUTE_JS */': (TEMPLATES / 'route-panorama.js').read_text(encoding='utf-8'),
            })
        html = template
        for marker, replacement in replacements.items():
            assert marker in html, f'Missing template marker: {name} {marker}'
            html = html.replace(marker, replacement)
        assert not any(marker in html for marker in replacements), f'Unrendered marker: {name}'
        target = ROOT / 'visuals' / f'{name}.html'
        if check:
            assert target.is_file() and target.read_text(encoding='utf-8') == html, f'Stale view: {target.name}'
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(html, encoding='utf-8')
        outputs.append(str(target.relative_to(ROOT)))
    for name in ('architecture-overview', 'implementation-progress', 'kotlin-swift-architecture', 'kotlin-arkts-architecture', 'route-atlas'):
        emit(name, name, dataset)
    for view in views:
        emit(view['slug'], 'route-panorama', dict(dataset, view=view))
    print(json.dumps(dict(mode='check' if check else 'render', tasks=len(plan['tasks']), routes=len(routes['routes']), nodes=len(architecture['nodes']), profiles=sum(len(view['profiles']) for view in views), outputs=outputs),ensure_ascii=False))

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true',help='Fail if committed HTML differs from catalog rendering')
    render(parser.parse_args().check)
