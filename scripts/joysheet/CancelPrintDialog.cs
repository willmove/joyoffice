// Windows integration test only: .NET Framework supplies Win32 UIA client-side providers.
using System;
using System.Diagnostics;
using System.Threading;
using System.Windows.Automation;
class CancelPrintDialog {
    static int Main(string[] args) {
        try {
            int id = int.Parse(args[0]);
            Process helper = Process.GetProcessById(id);
            DateTime deadline = DateTime.UtcNow.AddSeconds(10);
            do {
                helper.Refresh();
                AutomationElement root = AutomationElement.FromHandle(helper.MainWindowHandle);
                if (root != null && root.Current.ProcessId == id) {
                    AutomationElement cancel = root.FindFirst(TreeScope.Descendants, new AndCondition(
                        new PropertyCondition(AutomationElement.AutomationIdProperty, "2"),
                        new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.Button)));
                    if (cancel != null) {
                        ((InvokePattern)cancel.GetCurrentPattern(InvokePattern.Pattern)).Invoke();
                        Console.WriteLine("Native Cancel invoked");
                        return 0;
                    }
                }
                Thread.Sleep(100);
            } while (DateTime.UtcNow < deadline);
            throw new InvalidOperationException("Owned print dialog did not expose its Cancel button");
        } catch (Exception e) { Console.Error.WriteLine(e.Message); return 1; }
    }
}
