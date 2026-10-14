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
pub struct RasterTicket<K = RasterRequestKey> {
    serial: u64,
    key: K,
}
impl<K> RasterTicket<K> {
    pub fn key(&self) -> &K {
        &self.key
    }
}
pub struct RasterRequestGate<K = RasterRequestKey> {
    requested: Option<RasterTicket<K>>,
    serial: u64,
}
impl<K> Default for RasterRequestGate<K> {
    fn default() -> Self {
        Self {
            requested: None,
            serial: 0,
        }
    }
}
impl<K: Clone + PartialEq> RasterRequestGate<K> {
    pub fn request(&mut self, key: K) -> Option<RasterTicket<K>> {
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
    pub fn accepts(&self, ticket: &RasterTicket<K>, current: &K) -> bool {
        self.requested.as_ref() == Some(ticket) && &ticket.key == current
    }
}
