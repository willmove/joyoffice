"""Compile the owned Windows print/media adapters from reviewed source; no downloads."""
from pathlib import Path
import argparse,os,platform,subprocess,sys,json,hashlib
ROOT=Path(__file__).resolve().parents[2]
def build(output):
    if sys.platform!='win32':return []
    framework=Path(os.environ.get('WINDIR','C:/Windows'))/'Microsoft.NET/Framework64/v4.0.30319'
    compiler=framework/'csc.exe'
    if not compiler.is_file():raise ValueError('Windows .NET Framework compiler unavailable; helper build refused')
    # The framework compiler predates an ARM64 platform flag. IL-only AnyCPU
    # helpers can run on the native 64-bit framework on Windows on Arm.
    architecture = 'anycpu' if platform.machine().lower() == 'arm64' else 'x64'
    output=Path(output);output.mkdir(parents=True,exist_ok=True);binaries=[]
    for name,refs in [('print',['System.Drawing.dll','System.Windows.Forms.dll','System.Web.Extensions.dll']),('media',['System.Web.Extensions.dll','WPF/WindowsBase.dll','WPF/PresentationCore.dll'])]:
        target=output/f'joydeck-{name}-windows.exe'
        subprocess.run([str(compiler),'/nologo','/target:exe','/optimize+',f'/platform:{architecture}',f'/out:{target}',*[f'/reference:{framework/ref}' for ref in refs],str(ROOT/f'crates/joydeck-shell/native/{name}-windows.cs')],check=True)
        binaries.append(target)
    sdk=Path(os.environ.get('ProgramFiles(x86)','C:/Program Files (x86)'))/'Windows Kits/10/UnionMetadata'
    metadata=sorted(p for p in sdk.glob('*/Windows.winmd') if p.parent.name[0].isdigit())
    if not metadata:raise ValueError('Windows SDK WinRT metadata unavailable; MP4 adapter build refused')
    target=output/'joydeck-video-windows.exe'
    refs=[framework/'System.Web.Extensions.dll',framework/'System.Runtime.WindowsRuntime.dll',framework/'System.Runtime.dll',framework/'System.Threading.Tasks.dll',metadata[-1]]
    subprocess.run([str(compiler),'/nologo','/target:exe','/optimize+',f'/platform:{architecture}',f'/out:{target}',*[f'/reference:{p}' for p in refs],str(ROOT/'crates/joydeck-shell/native/video-windows.cs')],check=True)
    binaries.append(target)
    def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
    (output/'native-helpers.json').write_bytes((json.dumps(dict(compiler=str(compiler),compiler_sha256=sha(compiler),sdk_metadata=str(metadata[-1]),sdk_metadata_sha256=sha(metadata[-1]),system_framework='Windows .NET Framework 4.x / WPF / WinForms / WinRT',binaries={p.name:sha(p) for p in binaries},sources={p.name:sha(p) for p in (ROOT/'crates/joydeck-shell/native').glob('*.cs')}),indent=2)+'\n').encode())
    return binaries
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args();print([str(x) for x in build(args.output)])
