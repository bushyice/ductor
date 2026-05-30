use ductor::*;

#[cap(derive(Debug, Clone, Copy))]
pub struct ReadCap;

#[cap(derive(Debug, Clone, Copy))]
pub struct WriteCap;

#[cap(derive(Debug, Clone, Copy))]
pub struct AdminCap;

#[typestate(derive(Debug, Clone))]
pub enum DocState {
  #[transition(Open, requires = (ReadCap, WriteCap))]
  Closed,

  #[transition(Open)]
  Open { content: String },
}

#[unit(derive(Debug, Clone), states = DocState)]
pub struct Doc<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = Open)]
impl Doc {
  pub fn read(&self) -> &str {
    &self.state.select(DocState).content
  }
}

#[spec(for = Open, with = WriteCap)]
impl Doc {
  pub fn write(self, new_content: &str) -> Self {
    self.transition(|Open { .. }| Open {
      content: new_content.to_string(),
    })
  }
}

#[spec(for = Open, with = AdminCap)]
impl Doc {
  pub fn get_metadata(&self) -> &'static str {
    "size: 42, owner: admin"
  }
}

fn main() {
  let caps = caps!(ReadCap, WriteCap);
  let doc = Doc::new(Closed, caps)
    .transition(|Closed| Open {
      content: String::new(),
    })
    .write("Hello, capabilities!");

  println!("Content: {}", doc.read());
}
