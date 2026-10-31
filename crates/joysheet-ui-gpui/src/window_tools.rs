use crate::desktop::{Desktop, NativeWindows};
use gpui_kit::*;
use serde_json::{Value, json};
impl Desktop {
    pub fn apply_window_arrangement(&mut self, value: &Value, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mode) = value.get("arrange").and_then(Value::as_str) else { return };
        let result = (|| -> Result<Value, String> {
            let ids = value["windows"].as_array().ok_or("Invalid window arrangement response")?;
            let mut handles = Vec::new();
            let mut windows = Vec::new();
            for id in ids {
                let id = id.as_u64().ok_or("Invalid window identity")?;
                let handle = cx.global::<NativeWindows>().2.get(&id).copied().ok_or("A native window is not ready; retry after opening")?;
                windows.push(id);
                handles.push(handle);
            }
            let actual = joysheet_platform::windows::arrange(&handles, mode)?;
            Ok(json!({"mode":mode,"windows":windows.into_iter().zip(actual).map(|(id,info)|json!({"window":id,"native":info})).collect::<Vec<_>>()}))
        })();
        match result {
            Ok(value) => {
                self.last_arrangement = value;
                window.activate_window();
            }
            Err(error) => {
                self.last_arrangement = json!({"error":error});
                self.alert(error);
            }
        }
        cx.notify();
    }
    pub fn native_windows(&self, cx: &App) -> Value {
        let mapping = &cx.global::<NativeWindows>().2;
        let handles: Vec<_> = self.snapshot.windows.iter().filter_map(|v| mapping.get(&v.window).copied()).collect();
        match joysheet_platform::windows::inspect(&handles) {
            Ok(info) => json!(
                info.into_iter()
                    .map(|native| json!({"window":mapping.iter().find(|(_,h)|**h==native.handle).map(|(id,_)|*id),"native":native}))
                    .collect::<Vec<_>>()
            ),
            Err(e) => json!({"error":e}),
        }
    }
}
