use ductor::*;

// individual caps

#[cap(derive(Debug, Clone))]
pub struct Read;

#[cap(derive(Debug, Clone))]
pub struct Write;

#[cap(derive(Debug, Clone))]
pub struct Delete;

// parent cap: delegates to Read + Write + Delete
//
// `Admin` satisfies requirement bounds for Read, Write, and Delete.
// just `caps!(Admin)` is enough now.

#[cap(as = (Read, Write, Delete))]
pub struct Admin;

// state machine:
//
//   Closed ──(needs Read)──▶ Open
//     ▲                      │
//     │        ┌─────────────┤
//     │  (needs Delete)      │ (needs Write)
//     │        ▼             ▼
//     └───────Close◀─────────┘

#[typestate(derive(Debug, Clone))]
pub enum FileState {
  #[transition(Open, requires = Read)]
  Closed,

  #[transition(Close, requires = Write)]
  #[transition(Open)]
  Open { content: String },

  #[transition(Open, requires = Delete)]
  Close { content: String },
}

#[unit(derive(Debug, Clone), states = FileState)]
pub struct File<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = Closed, with = Read)]
impl File {
  #[transit(to = Open)]
  pub fn open(self, content: &str) -> Self {
    self.transition(|Closed| Open {
      content: content.to_string(),
    })
  }
}

#[spec(for = Open)]
impl File {
  pub fn read(&self) -> &str {
    &self.state.select(FileState).content
  }
}

#[spec(for = Open, with = Write)]
impl File {
  #[transit(to = Close)]
  pub fn close(self) -> Self {
    self.transition(|Open { content }| Close { content })
  }
}

#[spec(for = Close, with = Delete)]
impl File {
  #[transit(to = Open)]
  pub fn reopen(self, extra: &str) -> Self {
    self.transition(|Close { mut content }| {
      content.push_str(extra);
      Open { content }
    })
  }
}

fn main() {
  // using each caps
  let caps = caps!(Read, Write, Delete);
  let file = File::new(Closed, caps)
    .open("hello")
    .close()
    .reopen(" world");

  println!("Individual caps: {}", file.read());

  // using Admin cap instead of Read + Write + Delete
  // Admin proxies all three, so this compiles fine without them individually.
  let admin_caps = caps!(Admin);
  let file = File::new(Closed, admin_caps)
    .open("admin says")
    .close()
    .reopen(" proxy works");

  println!("Admin caps:     {}", file.read());
}
