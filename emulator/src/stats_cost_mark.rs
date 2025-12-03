use crate::StatsCosts;

#[derive(Clone, Debug)]
pub struct StatsCostMark {
    pub start: Option<StatsCosts>,
    pub costs: Vec<StatsCosts>,
}

impl StatsCostMark {
    pub fn new() -> Self {
        Self { start: None, costs: Vec::new() }
    }
}
