"""Create an unsigned native development package without replacing older stages."""
import argparse
import re
import hashlib
import json
import platform
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--name', help='New package directory name; existing packages are preserved')
    parser.add_argument('--stage', choices=['m4','m5','m6','m7','m8','m9','m10','m11','m12','m13','m14'], default='m14')
    parser.add_argument('--profile', choices=['release','debug'], default='release')
    args = parser.parse_args()
    if args.name and not re.fullmatch(r'joydeck-[a-z0-9-]+', args.name):
        raise ValueError('Package name must match joydeck-[a-z0-9-]+')
    build=['cargo','build','--locked','-p','joydeck']
    if args.profile=='release':build.append('--release')
    subprocess.run(build,cwd=ROOT,check=True)
    system, arch = platform.system().lower(), platform.machine().lower()
    name = args.name or f'joydeck-{args.stage}-{system}-{arch}-{args.profile}'
    output = ROOT / 'dist' / name
    if output.exists():
        raise FileExistsError(f'Preserve the existing package or choose a new output name: {output}')
    output.mkdir(parents=True)
    binary = 'joydeck.exe' if system == 'windows' else 'joydeck'
    shutil.copy2(ROOT / 'target' / args.profile / binary, output / binary)
    cli='joydeck-cli.exe' if system=='windows' else 'joydeck-cli'
    shutil.copy2(ROOT/'target'/args.profile/cli, output/cli)
    if system=='darwin':
        for helper in ['media','print','video']:
            source=ROOT/'apps/joydeck/native'/f'{helper}-macos.swift'
            executable=output/f'joydeck-{helper}-macos'
            subprocess.run(['swiftc','-O',str(source),'-o',str(executable)],check=True)
    for path in ['LICENSE-MIT' , 'LICENSE-APACHE', 'THIRD_PARTY.md', 'Cargo.lock', 'rust-toolchain.toml']:
        shutil.copy2(ROOT / path, output / path)
    shutil.copy2(ROOT / f'packaging/README-{args.stage}.md', output / 'README.md')
    shutil.copytree(ROOT / 'licenses', output / 'licenses')
    shutil.copytree(ROOT / 'tests/fixtures', output / 'fixtures')
    (output / 'scripts').mkdir()
    for script in ['smoke-native.py','smoke-macos-ime.py','macos-input-source.c','benchmark-m4.py','verify-m5.py','verify-m6.py','verify-m7.py','verify-m8.py','verify-m9-m10.py','verify-m11-m12.py','verify-automation.py','bench-media.py','verify-m13-m14.py','inspect-video.swift','release-tool.py','sbom.py','verify-release.py','verify-corpus.py','verify-macos-integration.py','fetch-license-notices.py']:
        shutil.copy2(ROOT / 'scripts' / script,output / 'scripts' / script)
    if system == 'darwin':
        import plistlib
        app=output/'JoyDeck.app/Contents'
        (app/'MacOS').mkdir(parents=True)
        (app/'Info.plist').write_bytes(plistlib.dumps({'CFBundleExecutable':'JoyDeck','CFBundleIdentifier':f'dev.joydeck.preview.{args.stage}','CFBundleName':'JoyDeck','CFBundlePackageType':'APPL','CFBundleVersion':args.stage[1:],'CFBundleShortVersionString':'0.1.0','NSHighResolutionCapable':True,'CFBundleDocumentTypes':[{'CFBundleTypeName':'JoyDeck Document','CFBundleTypeRole':'Editor','LSHandlerRank':'Owner','CFBundleTypeExtensions':['joydeck'],'LSItemContentTypes':['dev.joydeck.document']},{'CFBundleTypeName':'Office Presentation Import','CFBundleTypeRole':'Viewer','LSHandlerRank':'Alternate','CFBundleTypeExtensions':['pptx','potx','ppsx']}],'UTExportedTypeDeclarations':[{'UTTypeIdentifier':'dev.joydeck.document','UTTypeDescription':'JoyDeck Document','UTTypeConformsTo':['public.zip-archive'],'UTTypeTagSpecification':{'public.filename-extension':['joydeck']}}]}))
        launcher=app/'MacOS/JoyDeck'
        shutil.copy2(output/binary,launcher)
        for executable in [cli,'joydeck-media-macos','joydeck-print-macos','joydeck-video-macos']:
            shutil.copy2(output/executable,app/'MacOS'/executable)
        resources=app/'Resources';resources.mkdir()
        shutil.copy2(output/'README.md',resources/'README.md')
        shutil.copy2(output/'THIRD_PARTY.md',resources/'THIRD_PARTY.md')
        shutil.copytree(output/'licenses',resources/'licenses')
        launch = output / 'Launch.command'
        launch.write_text('#!/bin/sh\ncd -- "$(dirname -- "$0")" || exit 1\nexec ./joydeck\n')
        launch.chmod(0o755)
    import sbom
    rustc=subprocess.check_output(['rustc','-vV'],cwd=ROOT,text=True)
    target=next(line.split(': ',1)[1] for line in rustc.splitlines() if line.startswith('host:'))
    dependency_info=sbom.generate(output/'SBOM.spdx.json',target,args.profile)
    source_files=sorted([*ROOT.glob('apps/joydeck/src/**/*.rs'),*ROOT.glob('apps/joydeck/native/*.swift'),ROOT/'apps/joydeck/build.rs',ROOT/'apps/joydeck/Cargo.toml',ROOT/'Cargo.toml',ROOT/'Cargo.lock'])
    source_hashes={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in source_files}
    (output/'SOURCE-HASHES.json').write_text(json.dumps(source_hashes,indent=2)+'\n')
    info={'version':'0.1.0','stage':args.stage,'target':target,'profile':args.profile,'features':['desktop'],'nativeReaderMax':11,'writerSchema':'1.10','rustc':rustc,'cargoLockSha256':dependency_info['lockSha256'],'sourceGraphSha256':hashlib.sha256(json.dumps(source_hashes,sort_keys=True).encode()).hexdigest(),'dependencyPackages':dependency_info['packages'],'officialSignatureVerified':False}
    (output/'BUILD-INFO.json').write_text(json.dumps(info,indent=2)+'\n')
    if system=='darwin':
        resources=output/'JoyDeck.app/Contents/Resources'
        for name in ['BUILD-INFO.json','SBOM.spdx.json','SOURCE-HASHES.json']:shutil.copy2(output/name,resources/name)
        shutil.copytree(output/'dependency-licenses',resources/'dependency-licenses')
    hashes = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in sorted(output.rglob('*')) if p.is_file()}
    (output / 'SHA256.json').write_text(json.dumps(hashes, indent=2) + '\n')
    import zipfile
    archive=str(output)+'.zip'
    with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6,strict_timestamps=False) as zipped:
        for path in sorted(output.rglob('*')):
            if path.is_file():zipped.write(path,path.relative_to(output.parent))
    print(archive)

if __name__ == '__main__':
    main()
