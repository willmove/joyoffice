//! Child order for Office Math scripts (ECMA-376 Part 1, §22.1.2).
#[derive(Clone, Copy)]
pub enum Script {
    Sub,
    Sup,
    SubSup,
    Pre,
}

#[derive(Clone, Copy)]
pub enum ScriptArg {
    Base,
    Sub,
    Sup,
}

impl ScriptArg {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Base => "m:e",
            Self::Sub => "m:sub",
            Self::Sup => "m:sup",
        }
    }
}

impl Script {
    pub fn tags(self) -> (&'static str, &'static str) {
        match self {
            Self::Sub => ("m:sSub", "m:sSubPr"),
            Self::Sup => ("m:sSup", "m:sSupPr"),
            Self::SubSup => ("m:sSubSup", "m:sSubSupPr"),
            Self::Pre => ("m:sPre", "m:sPrePr"),
        }
    }

    pub fn arguments(self) -> &'static [ScriptArg] {
        use ScriptArg::{Base, Sub, Sup};
        match self {
            Self::Sub => &[Base, Sub],
            Self::Sup => &[Base, Sup],
            Self::SubSup => &[Base, Sub, Sup],
            Self::Pre => &[Sub, Sup, Base],
        }
    }

    /// Write XML for a script without optional control properties. The callback appends
    /// the already-escaped math content of each argument to the same buffer.
    pub fn write(self, out: &mut String, mut argument: impl FnMut(ScriptArg, &mut String)) {
        let (tag, _) = self.tags();
        open(out, tag);
        for &arg in self.arguments() {
            open(out, arg.tag());
            argument(arg, out);
            close(out, arg.tag());
        }
        close(out, tag);
    }
}

fn open(out: &mut String, tag: &str) {
    out.push('<');
    out.push_str(tag);
    out.push('>');
}

fn close(out: &mut String, tag: &str) {
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}
