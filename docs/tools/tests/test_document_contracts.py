"""Regression checks for documentation rules, not a browser or product test suite."""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import render_visuals as render
import check_docs

class DocumentationContracts(unittest.TestCase):
    def setUp(self):
        self.plan = render.read_json('implementation-plan.json')
        self.routes = render.read_json('routes.json')
        self.views = [json.loads((render.ROUTE_VIEWS/(slug+'.json')).read_text()) for slug in render.VIEW_SLUGS]
    def validate(self): render.validate_route_views(self.views, self.plan, self.routes)
    def test_current_projection(self): self.validate()
    def test_unknown_risk_owner_rejected(self):
        self.views[0]['profiles'][0]['risks'][0]['task_ids']=['C99']
        with self.assertRaises(AssertionError): self.validate()
    def test_unrelated_existing_task_rejected(self):
        self.views[0]['profiles'][0]['risks'][0]['task_ids']=['X09']
        with self.assertRaises(AssertionError): self.validate()
    def test_unknown_problem_rejected(self):
        self.views[0]['profiles'][0]['risks'][0]['problem_ids']=['TYPE-999']
        with self.assertRaises(AssertionError): self.validate()
    def test_small_explicit_graph_accepted(self):
        p=self.views[0]['profiles'][0]
        graph=p['detail']['graph']; graph['nodes']=graph['nodes'][:2]
        ids={n['id'] for n in graph['nodes']}
        graph['edges']=[e for e in graph['edges'] if e['a'] in ids and e['b'] in ids]
        p['detail']['models']=p['detail']['models'][:1]
        p['detail']['example']['code']='value = checkedCall()'
        self.validate()  # No requirement to invent 15 nodes or 4 lines.
    def test_compile_claim_requires_evidence(self):
        self.views[0]['profiles'][0]['detail']['example']['kind']='compile-checked'
        with self.assertRaises(AssertionError): self.validate()
    def test_build_steps_cannot_drift(self):
        self.views[0]['profiles'][0]['build_steps'][0]['text']='unrelated'
        with self.assertRaises(AssertionError): self.validate()
    def test_extension_requires_explicit_owner(self):
        self.routes['extension_work_packages']['knoi']=['C07']
        with self.assertRaises(AssertionError): render.validate(self.plan,render.read_json('architecture-map.json'),self.routes)
    def test_overlapping_nodes_rejected(self):
        nodes=self.views[0]['profiles'][0]['detail']['graph']['nodes']
        nodes[1]['x'],nodes[1]['y']=nodes[0]['x'],nodes[0]['y']
        with self.assertRaises(AssertionError): self.validate()
    def test_duplicate_json_key_rejected(self):
        with self.assertRaises(ValueError): json.loads('{"task":"C07","task":"X08"}',object_pairs_hook=check_docs.unique_object)

    def test_decision_unknown_task_rejected(self):
        self.plan['decision_gates'][0]['task_ids']=['C99']
        with self.assertRaises(AssertionError): render.validate_decision_gates(self.plan)
    def test_decision_missing_spec_rejected(self):
        self.plan['decision_gates'][0]['document']='architecture/missing.md'
        with self.assertRaises(AssertionError): render.validate_decision_gates(self.plan)
    def test_decision_duplicate_identity_rejected(self):
        self.plan['decision_gates'].append(copy.deepcopy(self.plan['decision_gates'][0]))
        with self.assertRaises(AssertionError): render.validate_decision_gates(self.plan)
    def test_decision_projection_escapes_text(self):
        self.plan['decision_gates'][0]['title']='<script>invalid</script>'
        html=render.decision_gate_rows(self.plan)
        self.assertIn('&lt;script&gt;invalid&lt;/script&gt;',html)
        self.assertNotIn('<script>',html)

    def test_project_tree_preserves_structure_and_escapes_markup(self):
        source='<!-- sample:start -->\n```text\nMadB/\n└── <extension-id> & sdk\n```\n<!-- sample:end -->'
        self.assertEqual(render.project_tree_html(source,'sample'),'MadB/\n└── &lt;extension-id&gt; &amp; sdk')
    def test_project_tree_rejects_ambiguous_or_malformed_source(self):
        valid='<!-- sample:start -->\n```text\nMadB/\n```\n<!-- sample:end -->'
        for source in ('', valid+valid, valid.replace('```text','```rust'), '<!-- sample:end -->'+valid.split('<!-- sample:end -->')[0]):
            with self.subTest(source=source), self.assertRaises(AssertionError):
                render.project_tree_html(source,'sample')

    def source_errors(self, coverage, resolve=None):
        root = TOOLS.parent
        return check_docs.source_coverage_errors(coverage, resolve or (lambda href: (root/'catalog'/href).resolve()))

    def test_archived_research_and_current_mapping_agree(self):
        self.assertEqual(self.source_errors(render.read_json('source-coverage.json')), [])

    def test_stale_research_location_rejected(self):
        coverage = render.read_json('source-coverage.json')
        coverage['mappings'][0]['line'] += 1
        self.assertTrue(any('location/title' in e for e in self.source_errors(coverage)))

    def test_research_mapping_cannot_disappear_or_duplicate(self):
        coverage = render.read_json('source-coverage.json')
        coverage['mappings'][0] = copy.deepcopy(coverage['mappings'][1])
        errors = self.source_errors(coverage)
        self.assertTrue(any('Duplicate historical mapping' in e for e in errors))
        self.assertTrue(any('cover source items exactly' in e for e in errors))

    def test_known_problem_cannot_silently_replace_original_mapping(self):
        coverage = render.read_json('source-coverage.json')
        coverage['mappings'][0]['problem_ids'] = ['TYPE-001']
        self.assertTrue(any('problem identity or mapping changed' in e for e in self.source_errors(coverage)))

    def test_damaged_archive_rejected(self):
        coverage = render.read_json('source-coverage.json')
        with tempfile.TemporaryDirectory() as directory:
            bad = Path(directory)/'invalid.zip'; bad.write_bytes(b'invalid archive')
            def resolve(href):
                return bad if href == coverage['archive_snapshot']['path'] else (TOOLS.parent/'catalog'/href).resolve()
            errors = self.source_errors(coverage, resolve)
            self.assertTrue(any('snapshot digest mismatch' in e for e in errors))
            self.assertTrue(any('snapshot invalid' in e for e in errors))

if __name__=='__main__': unittest.main()
