use crate::system::devices::MediaDeviceLists;

#[derive(Default)]
pub struct DeviceCache {
    lists: Option<MediaDeviceLists>,
    in_flight: bool,
}

impl DeviceCache {
    pub fn lists(&self) -> MediaDeviceLists {
        self.lists.clone().unwrap_or_default()
    }

    pub fn is_loaded(&self) -> bool {
        self.lists.is_some()
    }

    pub fn begin_refresh(&mut self) -> bool {
        if self.in_flight {
            return false;
        }
        self.in_flight = true;
        if self.lists.is_none() {
            self.lists = Some(MediaDeviceLists::default());
        }
        true
    }

    pub fn publish(&mut self, lists: MediaDeviceLists) {
        self.lists = Some(lists);
        self.in_flight = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::devices::MediaDevice;

    fn lists_with(id: &str) -> MediaDeviceLists {
        MediaDeviceLists {
            microphones: vec![MediaDevice {
                id: id.into(),
                label: id.into(),
            }],
            ..MediaDeviceLists::default()
        }
    }

    #[test]
    fn the_first_read_starts_one_query() {
        let mut cache = DeviceCache::default();
        assert!(!cache.is_loaded());
        assert!(cache.begin_refresh());
        assert!(cache.is_loaded());
        assert!(cache.lists().microphones.is_empty());
    }

    #[test]
    fn a_second_refresh_waits_for_the_one_in_flight() {
        let mut cache = DeviceCache::default();
        assert!(cache.begin_refresh());
        assert!(!cache.begin_refresh());
        cache.publish(lists_with("mic-1"));
        assert_eq!(cache.lists().microphones.len(), 1);
    }

    #[test]
    fn reopening_the_dropdown_requeries_and_replaces_the_list() {
        let mut cache = DeviceCache::default();
        assert!(cache.begin_refresh());
        cache.publish(lists_with("mic-1"));

        assert!(cache.begin_refresh());
        assert_eq!(cache.lists().microphones[0].id, "mic-1");
        cache.publish(lists_with("mic-2"));
        assert_eq!(cache.lists().microphones[0].id, "mic-2");
    }
}
