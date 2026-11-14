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
            if not candidates:
                raise ValueError('Missing package identity in selected graph: ' + name)
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
    # Keep the identity catalog unfiltered: cross-target build dependencies run
    # on the host. Target-only metadata can omit them. cargo tree below remains
    # the authority for selected normal/build packages and edges.
    data = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--offline',
         '--format-version', '1', *(['--features', ','.join(features)] if features else [])],
        cwd=ROOT, text=True, encoding='utf-8'))
    trees = []
    for package in roots:
        command = ['cargo', 'tree', '-p', package, '--locked', '--offline', '--target', target,
                   '--edges', 'normal,build', '--prefix', 'depth', '--no-dedupe', '--format', '{p}']
        selected_features = [f for f in features if '/' not in f or f.split('/', 1)[0] == package]
        if selected_features:
            command += ['--features', ','.join(selected_features)]
        trees.append(subprocess.check_output(command, cwd=ROOT, text=True, encoding='utf-8'))
    reached, edges = selected_graph(data, trees)
    packages = {package['id']: package for package in data['packages']}
    root_ids = {i for i in reached if packages[i]['name'] in roots}
    if {packages[i]['name'] for i in root_ids} != set(roots):
        raise ValueError('Selected graph does not contain every requested package root')
    return data, packages, reached, edges, root_ids


def source_hashes(data, packages, reached):
    files = {ROOT / name for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')}
    workspace = set(data['workspace_members'])
    local = {identifier for identifier in reached
             if not packages[identifier].get('source')
             and Path(packages[identifier]['manifest_path']).is_relative_to(ROOT)}
    for identifier in local:
        directory = Path(packages[identifier]['manifest_path']).parent
        files.add(directory / 'Cargo.toml')
        for folder in ('src', 'native', 'tests', 'examples'):
            files.update(p for p in (directory / folder).rglob('*')
                         if p.is_file() and p.suffix in ('.rs', '.swift', '.toml', '.c', '.h'))
        if (directory / 'build.rs').is_file():
            files.add(directory / 'build.rs')
        files.update(directory.glob('*.rc'))
    for product in {packages[i]['name'] for i in reached} & {'joyword', 'joydeck', 'joysheet'}:
        files.update(ROOT / 'assets/brand' / f'{product}.{extension}' for extension in ('svg', 'ico', 'icns'))
        files.update(ROOT / 'scripts' / script for script in ('product_icons.py', 'generate_product_icons.py'))
        files.update(ROOT / name for name in ('scripts/package_windows.py',
                     'scripts/windows/joyoffice.nsi', 'scripts/verify_release_assets.py'))
    is_word = any(packages[i]['name'] in ('joyword', 'joyword-cli') for i in reached)
    is_sheet = any(packages[i]['name'] == 'joysheet' for i in reached)
    if is_sheet:
        for identifier in reached & workspace:
            directory = Path(packages[identifier]['manifest_path']).parent
            files.update(p for p in directory.rglob('*')
                         if p.is_file() and not {'target', '__pycache__', '.git'} & set(p.relative_to(directory).parts))
        for folder in ('scripts/joysheet', 'docs/joysheet', 'tools/joysheet'):
            files.update(p for p in (ROOT / folder).rglob('*')
                         if p.is_file() and '__pycache__' not in p.parts)
        files.update(ROOT / name for name in ('AGENTS.md', 'deny.toml', '.cargo/config.toml',
                     'scripts/joydeck/sbom.py', 'scripts/joydeck/paths.py',
                     'tools/xtask/Cargo.toml', 'tools/xtask/src/main.rs'))
    elif is_word:
        # Word embeds workspace fonts, a welcome document and dictionaries.
        # These files and the shipped notices must follow the same snapshot
        # check as Rust input files.
        for folder in ('apps/joyword/assets', 'assets/fonts', 'assets/samples',
                       'third-party/craft-libs', 'third-party/craft-i18n',
                       'licenses/wordcraft', 'licenses/craft-libs',
                       'licenses/craft-i18n', 'licenses/gpui', 'licenses/accesskit-windows'):
            files.update(p for p in (ROOT / folder).rglob('*')
                         if p.is_file() and not {'target', '.git', '__pycache__'} & set(p.relative_to(ROOT).parts))
        files.update(ROOT / name for name in ('apps/joyword/LICENSE-NotoEmoji.txt',
                     'licenses/fonts/sources.json', 'licenses/joyoffice/JOYWORD-R8-SOURCE.json'))
        files.update(ROOT / 'scripts/joydeck' / name for name in ('sbom.py', 'paths.py'))
        files.add(ROOT / 'scripts/package_release.py')
    else:
        files.update(p for p in (ROOT / 'scripts/joydeck').glob('*.py'))
    files.update(p for p in (ROOT / 'tools/xtask').glob('*.py') if not p.name.startswith('test_'))
    return {p.relative_to(ROOT).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(files) if p.is_file()}


def snapshot(target, roots=('joydeck', 'joydeck-cli'), features=()):
    data, packages, reached, _, _ = graph(target, roots, features)
    return source_hashes(data, packages, reached)


def require_unchanged_sources(before, after):
    added = sorted(set(after) - set(before))
    removed = sorted(set(before) - set(after))
    changed = sorted(key for key in set(before) & set(after) if before[key] != after[key])
    if added or removed or changed:
        raise ValueError('Source inputs changed during the build; rebuild under a new output name. ' +
                         json.dumps({'added': added, 'removed': removed, 'changed': changed}))


def verified_supplement_files(folder):
    """Check recorded bytes; this does not establish license applicability or authenticity."""
    if folder.is_symlink():
        raise ValueError('Symlink dependency notice supplement: ' + str(folder))
    if not folder.is_dir():
        return {}
    files = {p.name: p for p in folder.iterdir() if p.is_file() or p.is_symlink()}
    if any(p.is_symlink() for p in files.values()):
        raise ValueError('Symlink in dependency notice supplement: ' + str(folder))
    manifest = files.get('SOURCES.json')
    if manifest is None:
        return {p: None for p in files.values()}
    if not 0 < manifest.stat().st_size <= 2 * 1024 * 1024:
        raise ValueError('Invalid notice provenance size: ' + str(manifest))
    raw = manifest.read_bytes()
    try:
        records = json.loads(raw)
    except (ValueError, UnicodeError) as error:
        raise ValueError('Invalid notice provenance JSON: ' + str(manifest)) from error
    if not isinstance(records, list) or not records:
        raise ValueError('Notice provenance must be a nonempty list: ' + str(manifest))
    hashes = {manifest: hashlib.sha256(raw).hexdigest()}
    names = {'SOURCES.json'}
    for record in records:
        if not isinstance(record, dict):
            raise ValueError('Invalid notice provenance record: ' + str(manifest))
        name, digest = record.get('file'), record.get('sha256')
        if (not isinstance(name, str) or not name or name in {'.', '..'}
                or any(c in name for c in '/\\:') or name in names
                or not isinstance(digest, str) or not re.fullmatch(r'[0-9a-fA-F]{64}', digest)):
            raise ValueError('Invalid or duplicate notice provenance file/hash: ' + str(manifest))
        names.add(name)
        path = files.get(name)
        if path is None or not 0 < path.stat().st_size <= 2 * 1024 * 1024:
            raise ValueError('Missing or invalid recorded dependency notice: ' + str(folder / name))
        digest = digest.lower()
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError('Dependency notice hash mismatch: ' + str(path))
        hashes[path] = digest
    if set(files) != names:
        raise ValueError('Unrecorded files in dependency notice supplement: ' + str(folder))
    return hashes


def generate(output, target, profile='release', roots=('joydeck', 'joydeck-cli'), features=(), require_license_texts=False):
    product = roots[0].split('-')[0] if roots else 'joydeck'
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
    for section in (ROOT / 'Cargo.lock').read_text(encoding='utf-8').split('[[package]]')[1:]:
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
            if p['name'] == 'joysheet' or p['name'].startswith('joysheet-'):
                candidates.extend(ROOT / 'docs/joysheet' / name for name in ('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE'))
            elif p['name'] == 'joydeck' or p['name'].startswith('joydeck-'):
                candidates.extend([LICENSES / 'LICENSE-MIT', LICENSES / 'LICENSE-APACHE'])
            else:
                candidates.append(ROOT / 'LICENSE')
        supplement = LICENSES / 'dependency-notices' / (p['name'] + '-' + p['version'])
        supplement_hashes = verified_supplement_files(supplement)
        candidates.extend(supplement_hashes)
        if product == 'joysheet':
            supplement = ROOT / 'docs/joysheet/third-party' / p['name']
            versioned = supplement / p['version']
            if versioned.is_dir():
                supplement = versioned
            supplement_hashes.update(verified_supplement_files(supplement))
            candidates.extend(supplement_hashes)
        copied = []
        for candidate in sorted(set(candidates)):
            if not candidate.is_file() or not 0 < candidate.stat().st_size <= 2 * 1024 * 1024:
                continue
            folder = license_root / (p['name'] + '-' + p['version'])
            folder.mkdir(exist_ok=True)
            relative = candidate.relative_to(source) if candidate.is_relative_to(source) else Path(candidate.name)
            destination = folder / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(candidate, destination)
            expected = supplement_hashes.get(candidate)
            if expected and hashlib.sha256(destination.read_bytes()).hexdigest() != expected:
                raise ValueError('Dependency notice hash mismatch after copy: ' + str(candidate))
            copied.append(destination.relative_to(output.parent).as_posix())
        license_index.append({'name': p['name'], 'version': p['version'], 'declared': p.get('license'),
                              'texts': sorted(set(copied)), 'workspace': p.get('source') is None})
        objects.append(row)
    missing = [row['name'] + '@' + row['version'] for row in license_index
               if not any(Path(name).name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'UNLICENSE'))
                          for name in row['texts'])]
    if require_license_texts and missing:
        raise ValueError('Missing dependency license texts: ' + ', '.join(missing))
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
    output.write_text(json.dumps(sbom, indent=2) + '\n', encoding='utf-8')
    (license_root / 'index.json').write_text(json.dumps(license_index, indent=2) + '\n', encoding='utf-8')
    return {'packages': len(objects), 'lockSha256': lock, 'target': target, 'profile': profile,
            'roots': list(roots), 'features': list(features),
            'licenseTextsVerified': require_license_texts,
            'missingLicenseTexts': missing,
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
