"""SPDX inventory for explicitly built product roots, platform and feature set."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import uuid
from paths import ROOT, LICENSES


def selected_graph(data, trees):
    """Use cargo tree's selected normal/build graph, not workspace feature union."""
    identifiers = {}
    for package in data['packages']:
        key = (package['name'], package['version'])
        identifiers.setdefault(key, []).append(package['id'])
    reached, edges = set(), set()
    for tree in trees:
        stack = []
        for line in tree.splitlines():
            match = re.match(r'^(\d+)(\S+) v([^\s]+)(?:\s|$)', line)
            if not match:
                raise ValueError('Unrecognized Cargo dependency tree row: ' + line)
            depth, name, version = int(match[1]), match[2], match[3]
            candidates = identifiers.get((name, version), [])
            if len(candidates) != 1:
                raise ValueError('Ambiguous package identity in selected graph: ' + name)
            identifier = candidates[0]
            reached.add(identifier)
            stack = stack[:depth]
            if depth:
                if len(stack) != depth:
                    raise ValueError('Invalid dependency depth')
                edges.add((stack[-1], identifier))
            stack.append(identifier)
    return reached, edges


def graph(target, roots=('joydeck', 'joydeck-cli'), features=()):
    data = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--offline', '--filter-platform', target,
         '--format-version', '1'], cwd=ROOT, text=True))
    trees = []
    for package in roots:
        command = ['cargo', 'tree', '-p', package, '--locked', '--offline', '--target', target,
                   '--edges', 'normal,build', '--prefix', 'depth', '--no-dedupe', '--format', '{p}']
        selected_features = [f for f in features if '/' not in f or f.split('/', 1)[0] == package]
        if selected_features:
            command += ['--features', ','.join(selected_features)]
        trees.append(subprocess.check_output(command, cwd=ROOT, text=True))
    reached, edges = selected_graph(data, trees)
    packages = {package['id']: package for package in data['packages']}
    root_ids = {i for i in reached if packages[i]['name'] in roots}
    if {packages[i]['name'] for i in root_ids} != set(roots):
        raise ValueError('Selected graph does not contain every requested package root')
    return data, packages, reached, edges, root_ids


def source_hashes(data, packages, reached):
    files = {ROOT / name for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')}
    workspace = set(data['workspace_members'])
    for identifier in reached & workspace:
        directory = Path(packages[identifier]['manifest_path']).parent
        files.add(directory / 'Cargo.toml')
        for folder in ('src', 'native', 'tests', 'examples'):
            files.update(p for p in (directory / folder).rglob('*')
                         if p.is_file() and p.suffix in ('.rs', '.swift', '.toml', '.c', '.h'))
        if (directory / 'build.rs').is_file():
            files.add(directory / 'build.rs')
    is_word = any(packages[i]['name'] == 'joyword' for i in reached)
    if is_word:
        files.update(ROOT / 'scripts/joydeck' / name for name in ('sbom.py', 'paths.py'))
        files.add(ROOT / 'scripts/package_release.py')
    else:
        files.update(p for p in (ROOT / 'scripts/joydeck').glob('*.py'))
    files.update(p for p in (ROOT / 'tools/xtask').glob('*.py') if not p.name.startswith('test_'))
    return {p.relative_to(ROOT).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(files) if p.is_file()}


def generate(output, target, profile='release', roots=('joydeck', 'joydeck-cli'), features=()):
    product = 'joyword' if roots and roots[0].startswith('joyword') else 'joydeck'
    data, packages, reached, edges, root_ids = graph(target, roots, features)
    ids = {}
    for identifier in reached:
        package = packages[identifier]
        origin = package.get('source') or Path(package['manifest_path']).relative_to(ROOT).as_posix()
        identity = package['name'] + '@' + package['version'] + '|' + origin
        ids[identifier] = 'SPDXRef-' + hashlib.sha256(identity.encode()).hexdigest()[:20]
    lock = hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest()
    now = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
    output = Path(output)
    license_root = output.parent / 'dependency-licenses'
    license_root.mkdir()
    checksums = {}
    for section in (ROOT / 'Cargo.lock').read_text().split('[[package]]')[1:]:
        record = {key: json.loads(value) for key, value in re.findall(r'^(name|version|source|checksum) = ("[^\n]*")$', section, re.M)}
        checksums[(record['name'], record['version'], record.get('source'))] = record.get('checksum')
    objects, license_index = [], []
    for identifier in sorted(reached):
        p = packages[identifier]
        checksum = checksums.get((p['name'], p['version'], p.get('source')))
        row = {'SPDXID': ids[identifier], 'name': p['name'], 'versionInfo': p['version'],
               'downloadLocation': ('https://crates.io/api/v1/crates/' + p['name'] + '/' + p['version'] + '/download')
               if (p.get('source') or '').startswith('registry+') else 'NOASSERTION',
               'filesAnalyzed': False, 'licenseDeclared': (p.get('license') or 'NOASSERTION').replace('/', ' OR '),
               'licenseConcluded': 'NOASSERTION', 'copyrightText': 'NOASSERTION'}
        if checksum:
            row['checksums'] = [{'algorithm': 'SHA256', 'checksumValue': checksum}]
        source = Path(p['manifest_path']).parent.resolve()
        candidates = [v for v in source.iterdir() if v.is_file() and not v.is_symlink()
                      and v.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'UNLICENSE'))]
        if p.get('license_file'):
            candidate = (source / p['license_file']).resolve()
            if candidate.is_relative_to(source) and candidate.is_file():
                candidates.append(candidate)
        if p.get('source') is None:
            if p['name'] == 'joydeck' or p['name'].startswith('joydeck-'):
                candidates.extend([LICENSES / 'LICENSE-MIT', LICENSES / 'LICENSE-APACHE'])
            else:
                candidates.append(ROOT / 'LICENSE')
        supplement = LICENSES / 'dependency-notices' / (p['name'] + '-' + p['version'])
        if supplement.is_dir():
            candidates.extend(v for v in supplement.iterdir() if v.is_file())
        copied = []
        for candidate in sorted(set(candidates)):
            if not candidate.is_file() or candidate.stat().st_size > 2 * 1024 * 1024:
                continue
            folder = license_root / (p['name'] + '-' + p['version'])
            folder.mkdir(exist_ok=True)
            relative = candidate.relative_to(source) if candidate.is_relative_to(source) else Path(candidate.name)
            destination = folder / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(candidate, destination)
            copied.append(destination.relative_to(output.parent).as_posix())
        license_index.append({'name': p['name'], 'version': p['version'], 'declared': p.get('license'),
                              'texts': sorted(set(copied)), 'workspace': p.get('source') is None})
        objects.append(row)
    relationships = [{'spdxElementId': 'SPDXRef-DOCUMENT', 'relationshipType': 'DESCRIBES',
                      'relatedSpdxElement': ids[i]} for i in sorted(root_ids)]
    relationships += [{'spdxElementId': ids[a], 'relationshipType': 'DEPENDS_ON',
                       'relatedSpdxElement': ids[b]} for a, b in sorted(edges)]
    sbom = {'spdxVersion': 'SPDX-2.3', 'dataLicense': 'CC0-1.0', 'SPDXID': 'SPDXRef-DOCUMENT',
            'name': product + ' dependency inventory ' + target,
            'documentNamespace': 'urn:uuid:' + str(uuid.uuid5(uuid.NAMESPACE_URL, product + '/' + target + '/' + lock + '/' + now)),
            'creationInfo': {'created': now, 'creators': ['Tool: joydeck-sbom-2']},
            'packages': objects, 'relationships': relationships,
            'comment': 'Selected Cargo normal/build graph for roots ' + ','.join(roots) +
                       '; target=' + target + '; features=' + ','.join(features) +
                       '. License declarations are metadata, not conclusions. System frameworks/fonts and QA tools are listed in THIRD_PARTY.md.'}
    output.write_text(json.dumps(sbom, indent=2) + '\n')
    (license_root / 'index.json').write_text(json.dumps(license_index, indent=2) + '\n')
    return {'packages': len(objects), 'lockSha256': lock, 'target': target, 'profile': profile,
            'roots': list(roots), 'features': list(features),
            'sourceHashes': source_hashes(data, packages, reached)}

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--target', required=True)
    parser.add_argument('--root', action='append', choices=['joydeck', 'joydeck-cli', 'joyword', 'joyword-cli'])
    parser.add_argument('--feature', action='append', default=[])
    args = parser.parse_args()
    result = generate(args.output, args.target, roots=tuple(args.root or ['joydeck', 'joydeck-cli']), features=args.feature)
    result.pop('sourceHashes')
    print(json.dumps(result, indent=2))
