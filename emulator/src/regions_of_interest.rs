use std::collections::BTreeMap;

use crate::{get_ops_costs, StatsCosts, MAIN_COST};

#[derive(Clone, Debug)]
pub struct CallerInfo {
    pub calls: usize,
    pub steps: usize,
}

#[derive(Clone, Debug)]
pub struct RegionsOfInterest {
    pub id: usize,
    pub from_pc: u32,
    pub to_pc: u32,
    pub name: String,
    costs: StatsCosts,
    pub calls: usize,
    pub callers: BTreeMap<usize, CallerInfo>,
    pub last_caller_index: Option<usize>,
    pub call_stack_rc: usize,
    call_stack_depth: Option<usize>,
}

impl RegionsOfInterest {
    pub fn new(id: usize, from_pc: u32, to_pc: u32, name: &str) -> Self {
        Self {
            id,
            from_pc,
            to_pc,
            costs: StatsCosts::new(),
            calls: 0,
            name: name.to_string(),
            callers: BTreeMap::new(),
            last_caller_index: None,
            call_stack_rc: 0,
            call_stack_depth: None,
        }
    }
    pub fn contains(&self, pc: u32) -> bool {
        pc >= self.from_pc && pc <= self.to_pc
    }
    pub fn caller_call(&mut self) {
        #[cfg(feature = "debug_stats")]
        println!(
            "\x1B[1;34mCALL_CALLER ROI[{}]:{} RC:{} => {}\x1B[0m",
            self.id,
            self.name,
            self.call_stack_rc,
            self.call_stack_rc + 1
        );
        self.call_stack_rc += 1;
    }
    pub fn update_call_depth(&mut self, call_stack_depth: usize) {
        if self.call_stack_depth.is_none() {
            self.call_stack_depth = Some(call_stack_depth);
        } else {
            self.call_stack_depth =
                Some(std::cmp::min(self.call_stack_depth.unwrap(), call_stack_depth));
        }
    }
    pub fn call(&mut self, caller: Option<usize>, call_stack_depth: usize) {
        self.calls += 1;
        self.update_call_depth(call_stack_depth);
        if let Some(caller_id) = caller {
            self.callers
                .entry(caller_id)
                .and_modify(|info| {
                    info.calls += 1;
                    info.steps += 1;
                })
                .or_insert(CallerInfo { calls: 1, steps: 1 });
            self.last_caller_index = Some(caller_id);
        }
    }
    pub fn return_call(&mut self, call_stack_depth: usize) {
        let _rc = self.call_stack_rc;
        if self.call_stack_rc > 0 {
            self.call_stack_rc -= 1;
        }
        self.update_call_depth(call_stack_depth);
        #[cfg(feature = "debug_stats")]
        println!(
            "\x1B[1;33mRETURN_CALL ROI:[{}]:{} RC:{} => {}\x1B[0m",
            self.id, self.name, _rc, self.call_stack_rc
        );
        assert!(_rc > self.call_stack_rc);
    }
    pub fn inc_step(&mut self) {
        if self.call_stack_rc == 0 {
            self.costs.steps += 1;
            if let Some(index) = self.last_caller_index {
                self.callers.entry(index).and_modify(|info| {
                    info.steps += 1;
                });
            }
        }
    }
    pub fn get_callers(&self) -> impl Iterator<Item = (&usize, &CallerInfo)> {
        self.callers.iter()
    }
    pub fn add_op(&mut self, op: u8) {
        if self.call_stack_rc == 0 {
            self.costs.ops[op as usize] += 1;
        }
    }
    pub fn update_costs(&mut self) {
        let (cost, precompiles_cost) = get_ops_costs(&self.costs.ops);
        self.costs.cost =
            cost + precompiles_cost + self.costs.mops.get_cost() + self.costs.steps * MAIN_COST;
    }
    pub fn get_cost(&self) -> u64 {
        self.costs.cost
    }
    pub fn get_mem_cost(&self) -> u64 {
        self.costs.mops.get_cost()
    }
    pub fn get_steps(&self) -> u64 {
        self.costs.steps
    }
    pub fn get_callstack_rc(&self) -> usize {
        self.call_stack_rc
    }
    pub fn memory_write(&mut self, address: u64, width: u64, value: u64) {
        if self.call_stack_rc == 0 {
            self.costs.mops.memory_write(address, width, value);
        }
    }
    pub fn memory_read(&mut self, address: u64, width: u64) {
        if self.call_stack_rc == 0 {
            self.costs.mops.memory_read(address, width);
        }
    }
    pub fn get_ops_costs(&self) -> &[u64; 256] {
        &self.costs.ops
    }
    pub fn get_call_stack_depth(&self) -> Option<usize> {
        self.call_stack_depth
    }
    pub fn add_delta_costs(&mut self, reference: &StatsCosts, current: &StatsCosts) -> Option<u64> {
        if self.call_stack_rc == 0 {
            Some(self.costs.add_delta(reference, current))
        } else {
            None
        }
    }
}
