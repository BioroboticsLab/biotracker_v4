use super::protocol::*;

impl Features {
    pub fn switch_ids(&mut self, switch_request: &EntityIdSwitch) {
        self.features.iter_mut().for_each(|f| {
            if f.id == Some(switch_request.id1) {
                f.id = Some(switch_request.id2);
            } else if f.id == Some(switch_request.id2) {
                f.id = Some(switch_request.id1);
            }
        });
    }
}
