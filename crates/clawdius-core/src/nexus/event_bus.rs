use crate::nexus::events::NexusEvent;

pub struct EventBus {
    history: Vec<NexusEvent>,
    max_history: usize,
}

impl EventBus {
    #[must_use]
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Vec::with_capacity(max_history),
            max_history,
        }
    }

    pub fn publish(&mut self, event: NexusEvent) {
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(event);
    }

    #[must_use]
    pub fn history(&self) -> &[NexusEvent] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::PhaseId;

    #[test]
    fn test_new() {
        let bus = EventBus::new(100);
        assert_eq!(bus.history().len(), 0);
    }

    #[test]
    fn test_default() {
        let bus = EventBus::default();
        assert_eq!(bus.history().len(), 0);
    }

    #[test]
    fn test_publish() {
        let mut bus = EventBus::new(10);
        bus.publish(NexusEvent::phase_started(PhaseId(0)));
        assert_eq!(bus.history().len(), 1);
    }

    #[test]
    fn test_max_history() {
        let mut bus = EventBus::new(3);
        for i in 0..5 {
            bus.publish(NexusEvent::phase_started(PhaseId(i)));
        }
        assert_eq!(bus.history().len(), 3);
    }

    #[test]
    fn test_clear_history() {
        let mut bus = EventBus::new(10);
        bus.publish(NexusEvent::phase_started(PhaseId(0)));
        bus.publish(NexusEvent::phase_started(PhaseId(1)));
        assert_eq!(bus.history().len(), 2);
        bus.clear_history();
        assert_eq!(bus.history().len(), 0);
    }
}
