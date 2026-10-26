"""Export the public L05 development sources for an independent consumer.

This is a source delivery, not a C10 candidate or a published package.
"""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[3]
FILES = {
    'StateObservation.ets': 'sdk/arkts/observation/StateObservation.ets',
    'ArkUIStateModel.ets': 'sdk/arkts/observation/ArkUIStateModel.ets',
    'README.md': 'sdk/arkts/observation/README.md',
    'LICENSE': 'LICENSE',
    'NOTICE': 'NOTICE',
}


def export(output: Path) -> dict:
    data = {}
    for name, source in FILES.items():
        path = ROOT / source
        if path.is_symlink() or not path.is_file():
            raise ValueError('public source must be a regular file: ' + source)
        data[name] = path.read_bytes()
    output.mkdir(parents=True, exist_ok=False)
    records = []
    for name, content in data.items():
        (output / name).write_bytes(content)
        records.append({'path': name, 'source': FILES[name], 'size': len(content),
                        'sha256': hashlib.sha256(content).hexdigest()})
    result = {'scope': 'L05 public development source delivery; not a C10 candidate',
              'files': records}
    (output / 'source-inputs.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    print(json.dumps(export(parser.parse_args().output), indent=2))
