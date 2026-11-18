"""Read shared ribbon groups through the OS accessibility tree of an isolated JoyOffice window."""
import argparse
import json
import platform
import subprocess
import time
from pathlib import Path

from native_pilot_support import ROOT, SheetControl, app, digest, port, wait
from probe_native_ime import mac_keys, mac_target, run, system_events


class NativeWindow:
    def __init__(self, proc, out):
        self.proc = proc
        self.mac = platform.system() == "Darwin"
        if self.mac:
            self.helper = out / "accessibility"
            subprocess.run(["swiftc", ROOT / "scripts/joydeck/unified-ui-ax.swift", "-o", self.helper], check=True)
            target = mac_target(proc.pid)
            wait(lambda: system_events(f"return exists window 1 of {target}") == "true", "visible window", proc)
            system_events(f"set frontmost of {target} to true")
        else:
            import gi
            gi.require_version("Atspi", "2.0")
            from gi.repository import Atspi
            self.atspi = Atspi
            self.window = wait(lambda: run("xdotool", "search", "--onlyvisible", "--pid", str(proc.pid)).splitlines()[0],
                               "visible X11 window", proc)
            run("xdotool", "windowactivate", "--sync", self.window)

    def size(self, width):
        if self.mac:
            target = mac_target(self.proc.pid)
            system_events(f"set size of window 1 of {target} to {{{width}, 740}}")
            return int(system_events(f"get size of window 1 of {target}").split(",")[0])
        run("xdotool", "windowsize", "--sync", self.window, str(width), "740")
        return int(dict(line.split("=", 1) for line in run("xdotool", "getwindowgeometry", "--shell", self.window).splitlines())["WIDTH"])

    def nodes(self):
        if self.mac:
            roots = json.loads(run(self.helper, str(self.proc.pid), "tree"))
            stack = [(node, []) for node in roots]
            while stack:
                node, parents = stack.pop()
                name = node.get("label") or node.get("title") or ""
                stack.extend((child, parents + [name]) for child in node.get("children", []))
                yield {**node, "parents":parents}
        else:
            from gi.repository import GLib
            context = GLib.MainContext.default()
            while context.pending():
                context.iteration(False)
            desk = self.atspi.get_desktop(0)
            roots = [desk.get_child_at_index(i) for i in range(desk.get_child_count())]
            stack = [(node, []) for node in roots if node and node.get_process_id() == self.proc.pid]
            while stack:
                obj, parents = stack.pop()
                if obj is None:
                    continue
                try:
                    obj.clear_cache()
                    node = {"role":obj.get_role_name(), "label":obj.get_name() or "", "id":obj.get_accessible_id() or "",
                            "enabled":obj.get_state_set().contains(self.atspi.StateType.ENABLED), "object":obj, "parents":parents}
                    stack.extend((obj.get_child_at_index(i), parents + [node["label"]]) for i in range(obj.get_child_count()))
                except GLib.Error:
                    # A ribbon rebuild can remove a node while its accessibility methods are in flight.
                    continue
                yield node

    def find(self, names, role=None, parent=None):
        return next((node for node in self.nodes() if names.intersection(node.get(k, "") for k in ("id","label","title"))
                     and (not parent or parent in node["parents"])
                     and (not role or role in node["role"].lower() or (role == "group" and node["role"] == "panel"))), None)

    def key(self, name):
        if self.mac:
            mac_keys(self.proc.pid, [{"collapse":"122 using command down", "escape":"53"}[name]])
        else:
            run("xdotool", "windowactivate", "--sync", self.window)
            if run("xdotool", "getactivewindow") != self.window:
                raise RuntimeError("lost focus on the test window")
            run("xdotool", "key", "--clearmodifiers", {"collapse":"ctrl+F1", "escape":"Escape"}[name])
        time.sleep(.55)

    def press(self, node):
        if self.mac:
            target = node.get("id") or node.get("label") or node["title"]
            run(self.helper, str(self.proc.pid), "press", target)
        else:
            action = node["object"].get_action_iface()
            actions = [action.get_action_name(i).lower() for i in range(action.get_n_actions())] if action else []
            click = next((i for i,name in enumerate(actions) if name in ("click","press","activate","default")), None)
            if click is None or not action.do_action(click):
                raise RuntimeError(f"cannot activate {node['label']}")
        time.sleep(.2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=("word","deck","sheet"))
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    number = port()
    profile = out / "profile"
    env = {f"JOY{args.product.upper()}_CONFIG_DIR":str(profile)}
    launch = {"sheet":["--control",str(number)], "word":["--",str(ROOT / "assets/samples/letter.docx")],
              "deck":[str(ROOT / "tests/fixtures/joydeck/independent.pptx")]}[args.product]
    rows = []
    with app(args.binary.resolve(), launch, out, env) as proc:
        surface = NativeWindow(proc, out)
        ctrl = SheetControl(number, profile) if args.product == "sheet" else None
        groups = {"Font","字体"} if args.product != "deck" else {"幻灯片","字体"}
        wait(lambda: surface.find(groups, "group"), "home ribbon group", proc)
        langs = ("zh","en") if args.product != "deck" else ("zh",)
        for lang in langs:
            if ctrl:
                ctrl.call("ui.set", language=lang)
            elif args.product == "word":
                surface.size(1180)
                time.sleep(.25)
                desired = "字体" if lang == "zh" else "Font"
                if not surface.find({desired}, "group"):
                    toggle = surface.find({"language","EN","中文"}, "button")
                    if not toggle:
                        raise RuntimeError("language control missing")
                    surface.press(toggle)
            for width in (1180,740):
                actual_width = surface.size(width)
                target = ({"字体"} if lang == "zh" else {"Font"}) if args.product != "deck" else groups
                def group():
                    return surface.find(target, "group")
                g = wait(group, f"{lang} visible group at {actual_width}", proc)
                if ctrl:
                    ctrl.call("ui.focus")
                    ctrl.call("ui.key", key="f1", ctrl=not surface.mac, cmd=surface.mac)
                else:
                    surface.key("collapse")
                wait(lambda: not group(), "keyboard collapses ribbon", proc)
                if ctrl:
                    ctrl.call("ui.key", key="f1", ctrl=not surface.mac, cmd=surface.mac)
                else:
                    surface.key("collapse")
                wait(group, "keyboard expands ribbon", proc)
                activated = False
                if args.product in ("word","sheet"):
                    names = ({"font-dialog", "字体设置", "Font Settings"} if not ctrl else
                             {"launcher-Font-home.formatCells", "打开格式设置", "Open formatting settings"})
                    launcher = wait(lambda: surface.find(names, "button", g.get("label") or g.get("title")), "font group launcher", proc)
                    assert str(launcher.get("enabled", "")).lower() in ("true","1")
                    surface.press(launcher)
                    if ctrl:
                        def font_dialog():
                            dialog = ctrl.call("ui.inspect").get("commandDialog", {})
                            return dialog.get("command") == "home.formatCells" and dialog.get("tab") == 2
                        wait(font_dialog, "Font tab opened by its launcher", proc)
                        ctrl.call("ui.dialog.cancel")
                        wait(lambda: not font_dialog(), "dismissed Format Cells", proc)
                    else:
                        cancel = wait(lambda: surface.find({"cancel-authoring","取消"}, "button"), "font form opened",proc)
                        surface.press(cancel)
                        wait(lambda: not surface.find({"cancel-authoring","取消"}, "button"), "font form dismissed",proc)
                    activated = True
                rows.append({"language":lang,"requestedWidth":width,"actualWidth":actual_width,
                             "group":g.get("label") or g.get("title"),"keyboardCollapseAndExpand":True,"fontLauncherOpenedAndDismissed":activated})
    (out / "report.json").write_text(json.dumps({"product":args.product,"binarySha256":digest(args.binary),
                                                  "cases":rows}, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
