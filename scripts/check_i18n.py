"""Reject Arabic UI text outside the dictionary and missing static translation keys."""
import json
from pathlib import Path
import re
import sys
ROOT=Path(__file__).resolve().parents[1]
labels=json.loads((ROOT/'src/i18n/ar.json').read_text())
def exists(key):
    value=labels
    for part in key.split('.'):
        if not isinstance(value,dict) or part not in value:
            return False
        value=value[part]
    return isinstance(value,str)
errors=[]
for path in (ROOT/'src').rglob('*'):
    if path.suffix not in ('.ts','.tsx'):
        continue
    text=path.read_text()
    if re.search('[ء-ي]',text):
        errors.append(f'{path.relative_to(ROOT)}: Arabic labels belong in ar.json')
    keys=re.findall(r"\bt\(['\"]([^'\"]+)['\"]\)",text)
    keys+=re.findall(r"(?:label|hint|title|submit)=[\"']([^\"']+)[\"']",text)
    keys+=re.findall(r"\b(?:label|hint|title|submit):\s*['\"]([^'\"]+)['\"]",text)
    for key in keys:
        if not exists(key):
            errors.append(f'{path.relative_to(ROOT)}: missing label {key}')
if errors:
    sys.exit('\n'.join(sorted(set(errors))))
print('Arabic labels are centralized and all static keys exist.')
