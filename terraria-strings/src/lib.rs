pub struct ItemIdMap(Vec<(u16, &'static str)>);

impl ItemIdMap {
    pub fn name_by_id(&self, id: u16) -> Option<&str> {
        self.0.iter().find(|pair| pair.0 == id).map(|pair| pair.1)
    }
    pub fn id_by_name(&self, name: &str) -> Option<u16> {
        self.0.iter().find(|pair| pair.1 == name).map(|pair| pair.0)
    }
}

pub fn item_ids() -> ItemIdMap {
    let mut item_ids = Vec::new();
    for line in ITEM_ID_LIST.lines() {
        let mut parts = line.split('\t');
        let id: u16 = parts.next().unwrap().parse().unwrap();
        let name = parts.next().unwrap();
        item_ids.push((id, name));
    }
    ItemIdMap(item_ids)
}

/// These are taken from https://terraria.fandom.com/wiki/Item_IDs
static ITEM_ID_LIST: &str = include_str!("../item_id_list.txt");
