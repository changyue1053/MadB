"""Independent schema/parser/JCS reference; no application implementation imports."""
from pathlib import Path
import copy
import hashlib
import json
import unittest

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/fixtures/contracts"
SCHEMAS = ROOT / "docs/schemas"


def read(name):
    return json.loads((FIXTURES / name).read_text())


def canonical(value):
    if isinstance(value, dict):
        return "{" + ",".join(json.dumps(key, ensure_ascii=False) + ":" + canonical(value[key])
                              for key in sorted(value, key=lambda s: s.encode("utf-16-be"))) + "}"
    if isinstance(value, list):
        return "[" + ",".join(map(canonical, value)) + "]"
    if isinstance(value, float):
        raise ValueError("FMT006")
    if type(value) is int and abs(value) > 9007199254740991:
        raise ValueError("FMT006")
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False)


def strict_decode(text):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("FMT005")
            result[key] = value
        return result

    def integer(text):
        if text == "-0" or abs(int(text)) > 9007199254740991:
            raise ValueError("FMT006")
        return int(text)

    def non_integer(_):
        raise ValueError("FMT006")

    try:
        value = json.loads(text, object_pairs_hook=pairs, parse_int=integer,
                           parse_float=non_integer, parse_constant=non_integer)
        canonical(value).encode("utf-8")  # rejects unpaired surrogates
        return value
    except (json.JSONDecodeError, UnicodeEncodeError) as error:
        raise ValueError("FMT001") from error


class ContractFormats(unittest.TestCase):
    def test_common_vectors_against_an_independent_schema_engine(self):
        schemas = {}
        registry = Registry()
        for path in SCHEMAS.glob("*.schema.json"):
            value = json.loads(path.read_text())
            if value.get("$id", "").startswith("urn:madb:schema:"):
                Draft202012Validator.check_schema(value)
                registry = registry.with_resource(value["$id"], Resource.from_contents(value))
                schemas[path.name.removesuffix(".schema.json")] = value
        seen = set()
        vectors = (read("format-vectors.json")["vectors"]
                   + read("composition-format-vectors.json")["vectors"]
                   + read("explain-format-vectors.json")["vectors"])
        for vector in vectors:
            with self.subTest(vector=vector["id"]):
                value = copy.deepcopy(read(vector["file"]))
                for patch in vector.get("patch", []):
                    parts = patch["path"].split("/")[1:]
                    parent = value
                    for key in parts[:-1]:
                        parent = parent[int(key)] if isinstance(parent, list) else parent[key]
                    key = int(parts[-1]) if isinstance(parent, list) else parts[-1]
                    if patch["op"] == "remove":
                        del parent[key]
                    else:
                        parent[key] = patch["value"]
                validator = Draft202012Validator(schemas[vector["kind"]], registry=registry)
                self.assertEqual(validator.is_valid(value), vector["expected"] == "valid")
                seen.add(vector["kind"])
        self.assertEqual(len(seen), 20)

    def test_jcs_bytes_and_hashes(self):
        vectors = read("canonical-vectors.json")
        for vector in vectors["vectors"]:
            with self.subTest(vector=vector["id"]):
                encoded = canonical(vector["input"])
                self.assertEqual(encoded, vector["canonical"])
                digest = hashlib.sha256((vectors["domain"] + "\n" + encoded).encode()).hexdigest()
                self.assertEqual("sha256:" + digest, vector["sha256"])

    def test_ambiguous_parser_input_rejected_before_schema(self):
        for vector in read("parse-vectors.json")["vectors"]:
            with self.subTest(vector=vector["id"]):
                with self.assertRaisesRegex(ValueError, vector["code"]):
                    strict_decode(vector["text"])


if __name__ == "__main__":
    unittest.main()
