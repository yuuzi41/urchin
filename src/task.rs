pub trait Task {
  fn get_cpu_affinity_mask(&self) -> u64 {
      0xffffffffffffffff
  }
  
  fn exec(&self);
}

#[derive(Debug, Copy, Clone)]
pub struct DummyTask;

impl Task for DummyTask {
  fn exec(&self) {
  }
}