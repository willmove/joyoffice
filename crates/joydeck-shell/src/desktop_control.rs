//! Explicit desktop stdio transport. The GUI thread executes each queued request.
use crate::automation::{LIMIT, Request};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    sync::mpsc,
};

pub struct Pending {
    pub request: Request,
    reply: mpsc::SyncSender<Value>,
}
impl Pending {
    pub fn respond(self, response: Value) {
        let _ = self.reply.send(response);
    }
}
pub fn listen() -> mpsc::Receiver<Pending> {
    let (send, receive) = mpsc::sync_channel::<Pending>(1);
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        let mut output = std::io::stdout().lock();
        loop {
            let mut line = vec![];
            match input
                .by_ref()
                .take((LIMIT + 1) as u64)
                .read_until(b'\n', &mut line)
            {
                Ok(0) | Err(_) => break,
                _ => {}
            }
            if line.len() > LIMIT {
                break;
            }
            let response = match serde_json::from_slice::<Request>(&line) {
                Err(error) => {
                    json!({"error":{"code":"invalid-request","message":error.to_string()}})
                }
                Ok(request) => {
                    let (reply, result) = mpsc::sync_channel(1);
                    if send.send(Pending { request, reply }).is_err() {
                        break;
                    }
                    match result.recv_timeout(std::time::Duration::from_secs(30)) {
                        Ok(result) => result,
                        Err(_) => break,
                    }
                }
            };
            if writeln!(output, "{response}")
                .and_then(|_| output.flush())
                .is_err()
            {
                break;
            }
        }
    });
    receive
}
