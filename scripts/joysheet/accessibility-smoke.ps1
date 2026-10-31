#Requires -Version 7.0
# Own-application UI Automation interoperability probe, never injects keyboard/mouse input.
param([Parameter(Mandatory)][long]$Handle,[Parameter(Mandatory)][int]$OwnerProcessId,[string]$Cell='B2',[switch]$Focus)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
$root=[System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$Handle)
if($null -eq $root -or $root.Current.ProcessId -ne $OwnerProcessId){throw 'UIA target is not the owned application'}
$deadline=[DateTime]::UtcNow.AddSeconds(10)
do {
    $all=$root.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
    $found=$null
    foreach($element in $all){if($element.Current.Name -eq $Cell){$found=$element;break}}
    if($found){break};Start-Sleep -Milliseconds 100
}while([DateTime]::UtcNow -lt $deadline)
if(!$found){throw "UIA did not expose visible cell $Cell"}
if($Focus){$found.SetFocus()}
$value=$null;$pattern=$null
if($found.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$pattern)){$value=$pattern.Current.Value}
$b=$found.Current.BoundingRectangle
@{passed=$true;source='Windows UI Automation / actual AccessKit platform provider';nodes=$all.Count;cell=$Cell;name=$found.Current.Name;value=$value;focusRequested=$Focus.IsPresent;bounds=@{x=$b.X;y=$b.Y;width=$b.Width;height=$b.Height};controlType=$found.Current.ControlType.ProgrammaticName}|ConvertTo-Json -Depth 5
