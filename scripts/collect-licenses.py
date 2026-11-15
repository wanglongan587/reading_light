"""Collect packaged dependency licenses; never copy source secrets or user data."""
import json
import re
import shutil
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent.parent
destination = root / 'dist' / 'windows-x64' / 'licenses'
destination.mkdir(parents=True, exist_ok=True)
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--format-version', '1', '--locked', '--filter-platform', 'x86_64-pc-windows-msvc'],
    cwd=root, text=True, encoding='utf-8'))
records = []

def copy_license(folder, label, declared, repository=None, archive=None):
    target = destination / re.sub(r'[^A-Za-z0-9_.-]', '_', label)
    found = []
    for path in folder.iterdir():
        if path.is_file() and path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')):
            target.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target / path.name)
            found.append(path.name)
    source_archive=None
    if archive and archive.exists() and (not found or declared and 'MPL' in declared):
        sources=destination/'sources'
        sources.mkdir(exist_ok=True)
        shutil.copy2(archive,sources/archive.name)
        source_archive='sources/'+archive.name
    if not found:
        for readme in folder.glob('README*'):
            if readme.is_file():
                target.mkdir(parents=True,exist_ok=True)
                shutil.copy2(readme,target/readme.name)
    records.append({'package': label, 'license': declared, 'files': found, 'repository':repository,'source_archive':source_archive})

for package in metadata['packages']:
    if package.get('source'):
        folder=Path(package['manifest_path']).parent
        archive=folder.parents[2]/'cache'/folder.parent.name/(folder.name+'.crate')
        copy_license(folder, package['name'] + '-' + package['version'], package.get('license'),package.get('repository'),archive)

modules = root / 'web' / 'node_modules'
lock = json.loads((root / 'web' / 'package-lock.json').read_text(encoding='utf-8'))
for path, info in lock.get('packages', {}).items():
    if path and not info.get('dev') and (root / 'web' / path).is_dir():
        copy_license(root / 'web' / path, 'npm-' + path.replace('node_modules/', '') + '-' + info['version'], info.get('license'))
for name in ('reader.js.LEGAL.txt',):
    path = root / 'web' / 'dist' / name
    if path.exists():
        shutil.copy2(path, destination / name)
(destination / 'DEPENDENCIES.json').write_text(json.dumps(records, ensure_ascii=False, indent=2), encoding='utf-8')
print(f'Collected license records for {len(records)} dependency packages.')
