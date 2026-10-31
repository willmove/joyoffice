#Requires -Version 7.0
param([Parameter(Mandatory)][string]$Directory,[Parameter(Mandatory)][string]$Output)
$ErrorActionPreference='Stop'
$workspace=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$payload=(Resolve-Path -LiteralPath $Directory).Path
if(!(Test-Path -LiteralPath (Join-Path $payload 'joysheet.exe'))){throw 'Missing desktop payload'}
$metadata=Get-Content -LiteralPath (Join-Path $payload 'BUILD.json') -Raw -Encoding utf8 | ConvertFrom-Json
$version=[string]$metadata.version
if($version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$'){throw 'Invalid product version'}
$displayName="JoySheet $version preview"
$compiler=$env:JOYSHEET_NSIS
if(!$compiler){$compiler=Join-Path $workspace 'artifacts/joysheet/tools/nsis-3.11/makensis.exe'}
if(!(Test-Path -LiteralPath $compiler)){$found=Get-Command makensis.exe -ErrorAction SilentlyContinue;if($found){$compiler=$found.Source}else{$compiler=Join-Path ${env:ProgramFiles(x86)} 'NSIS/makensis.exe'}}
if(!(Test-Path -LiteralPath $compiler)){throw 'Install NSIS or set JOYSHEET_NSIS to makensis.exe. See docs/joysheet/PACKAGING.md.'}
$vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if(Test-Path -LiteralPath $vswhere){
    $vs=& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if($vs){
        $redistributables=Get-ChildItem -LiteralPath (Join-Path $vs 'VC/Redist/MSVC') -Directory |Sort-Object Name -Descending
        foreach($redist in $redistributables){
            $crt=Get-ChildItem -LiteralPath (Join-Path $redist.FullName 'x64') -Directory -Filter 'Microsoft.VC*.CRT' -ErrorAction SilentlyContinue|Select-Object -First 1
            if($crt){Get-ChildItem -LiteralPath $crt.FullName -Filter '*.dll' |Copy-Item -Destination $payload;break}
        }
    }
}
$nsisNotice=Join-Path (Split-Path $compiler -Parent) 'COPYING'
if(Test-Path -LiteralPath $nsisNotice){New-Item -ItemType Directory -Path (Join-Path $payload 'licenses') -Force|Out-Null;Copy-Item -LiteralPath $nsisNotice -Destination (Join-Path $payload 'licenses/NSIS-COPYING.txt')}
$build=Join-Path $workspace 'artifacts/joysheet/packaging'
New-Item -ItemType Directory -Path $build -Force|Out-Null
$files=@(Get-ChildItem -LiteralPath $payload -Recurse -File)
function Escape-Nsis([string]$Text){return $Text.Replace('$','$$').Replace('"','$\"')}
$deletes=@($files|ForEach-Object {'Delete "$INSTDIR\'+(Escape-Nsis ([IO.Path]::GetRelativePath($payload,$_.FullName)))+'"'})
$dirs=@(Get-ChildItem -LiteralPath $payload -Recurse -Directory|Sort-Object {$_.FullName.Length} -Descending|ForEach-Object {'RMDir "$INSTDIR\'+(Escape-Nsis ([IO.Path]::GetRelativePath($payload,$_.FullName)))+'"'})
($deletes+$dirs)|Set-Content -LiteralPath (Join-Path $build 'uninstall-files.nsh') -Encoding utf8NoBOM
Copy-Item -LiteralPath (Join-Path $workspace 'scripts/joysheet/joysheet.nsi') -Destination $build
& $compiler /V2 "/DPAYLOAD=$payload" "/DOUTPUT=$([IO.Path]::GetFullPath($Output))" "/DICON=$workspace\assets\joysheet\joysheet.ico" "/DVERSION=$version" "/DDISPLAY_NAME=$displayName" (Join-Path $build 'joysheet.nsi')
if($LASTEXITCODE -ne 0){throw 'NSIS compilation failed'}
if($env:JOYSHEET_WINDOWS_SIGN_THUMBPRINT){
    $sign=Get-Command signtool.exe -ErrorAction Stop
    & $sign.Source sign /sha1 $env:JOYSHEET_WINDOWS_SIGN_THUMBPRINT /fd SHA256 /tr 'http://timestamp.digicert.com' /td SHA256 $Output
    if($LASTEXITCODE -ne 0){throw 'Signing installer failed'}
}
Get-Item -LiteralPath $Output |Select-Object Name,Length
