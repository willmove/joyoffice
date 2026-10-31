#Requires -Version 7.0
param([Parameter(Mandatory)][string]$Directory,[switch]$Validate,[double]$Width=793.7,[double]$Height=1122.5,[double]$OriginX=24,[double]$OriginY=24,[double]$MarginRight=24,[double]$MarginBottom=24,[int]$FromPage=1,[int]$ToPage=0,[string]$ExportXps,[int]$OwnerProcessId=0)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName PresentationFramework,PresentationCore,WindowsBase,System.Printing,ReachFramework
if(!$Validate -and $OwnerProcessId -gt 0){
    Add-Type -TypeDefinition @'
using System;
using System.Diagnostics;
public static class JoysheetPrintOwner {
    static Process owner;
    public static void Watch(int id) {
        owner=Process.GetProcessById(id);
        owner.Exited+=(sender,args)=>Environment.Exit(2);
        owner.EnableRaisingEvents=true;
        if(owner.HasExited) Environment.Exit(2);
    }
}
'@
    [JoysheetPrintOwner]::Watch($OwnerProcessId)
}
$pages=@(Get-ChildItem -LiteralPath $Directory -Filter 'page-*.png' | Sort-Object { [int]($_.BaseName -replace 'page-','') })
if($pages.Count -lt 1 -or $pages.Count -gt 32){throw 'Print requires between 1 and 32 rendered pages'}
function New-PrintDocument([double]$MediaWidth,[double]$MediaHeight,[double]$Left,[double]$Top,[double]$ExtentWidth,[double]$ExtentHeight,[int]$First,[int]$Last) {
    foreach($v in @($MediaWidth,$MediaHeight,$ExtentWidth,$ExtentHeight)){if(![double]::IsFinite($v) -or $v -le 0 -or $v -gt 20000){throw 'Invalid paper or printable dimensions'}}
    if(![double]::IsFinite($Left) -or ![double]::IsFinite($Top) -or $Left -lt 0 -or $Top -lt 0 -or $Left+$ExtentWidth -gt $MediaWidth+0.1 -or $Top+$ExtentHeight -gt $MediaHeight+0.1){throw 'Printable area lies outside the paper'}
    if($First -lt 1 -or $Last -gt $pages.Count -or $Last -lt $First){throw 'Invalid print page range'}
    $document=[System.Windows.Documents.FixedDocument]::new()
    for($i=$First-1;$i -lt $Last;$i++){
        $bitmap=[System.Windows.Media.Imaging.BitmapImage]::new()
        $bitmap.BeginInit();$bitmap.CacheOption=[System.Windows.Media.Imaging.BitmapCacheOption]::OnLoad
        $bitmap.UriSource=[uri]$pages[$i].FullName;$bitmap.EndInit();$bitmap.Freeze()
        if($bitmap.PixelWidth -lt 1 -or $bitmap.PixelHeight -lt 1 -or [long]$bitmap.PixelWidth*$bitmap.PixelHeight -gt 4000000){throw 'Rendered print page exceeds the image budget'}
        $image=[System.Windows.Controls.Image]::new();$image.Source=$bitmap
        $image.Width=$ExtentWidth;$image.Height=$ExtentHeight;$image.Stretch=[System.Windows.Media.Stretch]::Uniform
        [System.Windows.Documents.FixedPage]::SetLeft($image,$Left);[System.Windows.Documents.FixedPage]::SetTop($image,$Top)
        $page=[System.Windows.Documents.FixedPage]::new();$page.Width=$MediaWidth;$page.Height=$MediaHeight
        $null=$page.Children.Add($image)
        $content=[System.Windows.Documents.PageContent]::new();$content.Child=$page;$document.Pages.Add($content)
    }
    return $document
}
if($Validate){
    if($ToPage -eq 0){$ToPage=$pages.Count}
    $document=New-PrintDocument $Width $Height $OriginX $OriginY ($Width-$OriginX-$MarginRight) ($Height-$OriginY-$MarginBottom) $FromPage $ToPage
    $document.DocumentPaginator.ComputePageCount();$first=$document.DocumentPaginator.GetPage(0)
    if($ExportXps){
        $xps=[System.Windows.Xps.Packaging.XpsDocument]::new($ExportXps,[IO.FileAccess]::ReadWrite)
        try{[System.Windows.Xps.Packaging.XpsDocument]::CreateXpsDocumentWriter($xps).Write($document.DocumentPaginator)}finally{$xps.Close()}
    }
    @{outcome='validated';pages=$document.DocumentPaginator.PageCount;from=$FromPage;to=$ToPage;width=$first.Size.Width;height=$first.Size.Height;originX=$OriginX;originY=$OriginY;queueSubmitted=$false;xps=$ExportXps}|ConvertTo-Json -Compress
    exit 0
}
$dialog=[System.Windows.Controls.PrintDialog]::new();$dialog.UserPageRangeEnabled=$true
$dialog.MinPage=1;$dialog.MaxPage=$pages.Count
if($dialog.ShowDialog() -ne $true){@{outcome='cancelled';queueSubmitted=$false}|ConvertTo-Json -Compress;exit 0}
$cap=$dialog.PrintQueue.GetPrintCapabilities($dialog.PrintTicket)
$area=$cap.PageImageableArea
$media=$dialog.PrintTicket.PageMediaSize
$paperWidth=if($media.Width){[double]$media.Width}else{[double]$cap.OrientedPageMediaWidth}
$paperHeight=if($media.Height){[double]$media.Height}else{[double]$cap.OrientedPageMediaHeight}
# Capabilities report oriented media and imageable coordinates in 1/96-inch units.
if($cap.OrientedPageMediaWidth){$paperWidth=[double]$cap.OrientedPageMediaWidth}
if($cap.OrientedPageMediaHeight){$paperHeight=[double]$cap.OrientedPageMediaHeight}
if(!$area){throw 'Printer did not provide its imageable area'}
$from=1;$to=$pages.Count
if($dialog.PageRangeSelection -eq [System.Windows.Controls.PageRangeSelection]::UserPages){$from=$dialog.PageRange.PageFrom;$to=$dialog.PageRange.PageTo}
$document=New-PrintDocument $paperWidth $paperHeight $area.OriginWidth $area.OriginHeight $area.ExtentWidth $area.ExtentHeight $from $to
$dialog.PrintDocument($document.DocumentPaginator,'Joysheet')
@{outcome='submitted';queueSubmitted=$true;pages=$to-$from+1}|ConvertTo-Json -Compress

