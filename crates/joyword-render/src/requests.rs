//! Request identity at asynchronous view handoff; duplicate idle renders do
//! not schedule new jobs. Both successful and failed requests stay recorded.
use crate::Projection;
use joyword_layout::LayoutKey;

#[derive(Clone, Debug, PartialEq)]
pub struct RasterRequestKey {
    pub layout: LayoutKey,
    pub projection: Projection,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RasterTicket {
    serial: u64,
    key: RasterRequestKey,
}
impl RasterTicket {
    pub fn key(&self) -> &RasterRequestKey {
        &self.key
    }
}
#[derive(Default)]
pub struct RasterRequestGate {
    requested: Option<RasterTicket>,
    serial: u64,
}
impl RasterRequestGate {
    pub fn request(&mut self, key: RasterRequestKey) -> Option<RasterTicket> {
        if self.requested.as_ref().is_some_and(|old| old.key == key) {
            return None;
        }
        self.serial = self
            .serial
            .checked_add(1)
            .expect("render request generation exhausted");
        let ticket = RasterTicket {
            serial: self.serial,
            key,
        };
        self.requested = Some(ticket.clone());
        Some(ticket)
    }
    pub fn accepts(&self, ticket: &RasterTicket, current: &RasterRequestKey) -> bool {
        self.requested.as_ref() == Some(ticket) && &ticket.key == current
    }
}
