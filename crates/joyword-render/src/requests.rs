//! Request identity adapted from JoyOffice T108; overflow refuses scheduling.
#[derive(Clone, Debug, PartialEq)]
pub struct RasterTicket<K> {
    serial: u64,
    key: K,
}
impl<K> RasterTicket<K> {
    pub fn key(&self) -> &K {
        &self.key
    }
}
pub struct RasterRequestGate<K> {
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
        self.renew(key)
    }
    /// A new ticket even when `key` is the latest request, for callers that
    /// track completion themselves and no longer hold that request's result
    /// (it landed stale, or its pixels were evicted). Every earlier ticket,
    /// including the previous one for `key`, stops being accepted.
    pub fn renew(&mut self, key: K) -> Option<RasterTicket<K>> {
        self.serial = self.serial.checked_add(1)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superseded_and_cancelled_work_cannot_publish() {
        let mut gate = RasterRequestGate::default();
        let old = gate.request((1, 100)).unwrap();
        assert!(gate.request((1, 100)).is_none());
        let new = gate.renew((2, 150)).unwrap();
        assert!(!gate.accepts(&old, &(1, 100)));
        assert!(!gate.accepts(&new, &(2, 100)));
        assert!(gate.accepts(&new, &(2, 150)));
        let cancel = gate.renew((2, 150)).unwrap();
        assert!(!gate.accepts(&new, &(2, 150)));
        assert!(gate.accepts(&cancel, &(2, 150)));
    }

    #[test]
    fn serial_exhaustion_refuses_to_reuse_identity() {
        let mut gate = RasterRequestGate {
            serial: u64::MAX - 1,
            requested: None,
        };
        let last = gate.renew(1).unwrap();
        assert!(gate.renew(2).is_none());
        assert!(gate.accepts(&last, &1));
        assert!(!gate.accepts(&last, &2));
    }
}
