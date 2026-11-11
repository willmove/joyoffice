param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][string]$SdkDirectory,
    [Parameter(Mandatory)][string]$ReportPath
)
$ErrorActionPreference = 'Stop'
# Official NuGet packages: DocumentFormat.OpenXml + .Framework 3.0.1, System.IO.Packaging 8.0.0.
# Extract under SdkDirectory; these validation-only packages are never product dependencies.
foreach ($relative in @(
    'system.io.packaging/lib/net8.0/System.IO.Packaging.dll',
    'documentformat.openxml.framework/lib/net8.0/DocumentFormat.OpenXml.Framework.dll',
    'documentformat.openxml/lib/net8.0/DocumentFormat.OpenXml.dll'
)) { Add-Type -Path (Join-Path $SdkDirectory $relative) }
$document = [DocumentFormat.OpenXml.Packaging.SpreadsheetDocument]::Open((Resolve-Path -LiteralPath $Path), $false)
try {
    $validator = [DocumentFormat.OpenXml.Validation.OpenXmlValidator]::new([DocumentFormat.OpenXml.FileFormatVersions]::Microsoft365)
    $findings = @($validator.Validate($document) | ForEach-Object {
        [pscustomobject]@{ id=$_.Id; description=$_.Description; part=$_.Part.Uri.ToString(); path=$_.Path.XPath }
    })
    # SDK 3.0.1 models a14:m as a leaf, whereas MS-ODRAWXML §2.3.3.41 requires m:oMath.
    # Validate each OfficeMath subtree independently, retain the SDK discrepancy in the report.
    $mathCount = 0
    $mathErrors = @()
    $mathWrappersValid = $true
    foreach ($worksheet in $document.WorkbookPart.WorksheetParts) {
        $drawing = $worksheet.DrawingsPart
        if ($null -eq $drawing) { continue }
        $xml = [System.Xml.XmlDocument]::new()
        $xml.XmlResolver = $null
        $stream = $drawing.GetStream()
        try { $xml.Load($stream) } finally { $stream.Dispose() }
        $ns = [System.Xml.XmlNamespaceManager]::new($xml.NameTable)
        $ns.AddNamespace('a14','http://schemas.microsoft.com/office/drawing/2010/main')
        foreach ($wrapper in $xml.SelectNodes('//a14:m',$ns)) {
            if ($wrapper.ChildNodes.Count -ne 1 -or $wrapper.FirstChild.LocalName -ne 'oMath' -or $wrapper.FirstChild.NamespaceURI -ne 'http://schemas.openxmlformats.org/officeDocument/2006/math') {
                $mathWrappersValid = $false
                continue
            }
            $math = [DocumentFormat.OpenXml.Math.OfficeMath]::new($wrapper.FirstChild.OuterXml)
            $mathCount++
            $mathErrors += @($validator.Validate($math) | ForEach-Object {
                [pscustomobject]@{ id=$_.Id; description=$_.Description; part=$drawing.Uri.ToString(); path=$_.Path.XPath }
            })
        }
    }
    $sdkLimitations = @()
    if ($mathWrappersValid -and $mathCount -gt 0) {
        $sdkLimitations = @($findings | Where-Object { $_.id -eq 'Sch_InvalidChildinLeafElement' -and $_.path.Contains('/a14:m[') -and $_.description.Contains('http://schemas.microsoft.com/office/drawing/2010/main:m') })
        $findings = @($findings | Where-Object { $sdkLimitations -notcontains $_ })
    }
    $findings += $mathErrors
    $report = [pscustomobject]@{ file=(Resolve-Path -LiteralPath $Path).Path; sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash; sdk='3.0.1'; findings=$findings; sdkLimitations=$sdkLimitations; ommlSubtreesValidated=$mathCount; passed=($findings.Count -eq 0) }
    $report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ReportPath -Encoding utf8
    $findings | Select-Object -First 30 | ConvertTo-Json -Depth 6
    Write-Output "Open XML findings: $($findings.Count); separately validated OMML: $mathCount; recorded SDK limitations: $($sdkLimitations.Count)"
    if ($findings.Count -gt 0) { exit 1 }
} finally { $document.Dispose() }
