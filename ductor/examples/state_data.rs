use ductor::*;
use std::fmt;

#[typestate(derive(Debug, Clone))]
pub enum DocumentState {
  // nothing to see here, move along
  #[transition(Drafted)]
  Blank,

  // now we've got actual content
  #[transition(Reviewed)]
  Drafted { title: String, body: String },

  // reviewer had some thoughts
  #[transition(Published)]
  Reviewed {
    title: String,
    body: String,
    reviewer_note: Option<String>,
  },

  // terminal state, can't go anywhere else
  Published {
    title: String,
    body: String,
    reviewer_note: Option<String>,
  },
}

#[cap(derive(Debug, Clone))]
pub struct ReviewerRights;

#[unit(derive(Debug, Clone), states = DocumentState)]
pub struct Document<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = Blank)]
impl Document {
  #[transit(to = Drafted)]
  pub fn draft(self, title: &str, body: &str) {
    // `|_|` ignores the Blank state (it's a unit struct, no fields)
    self.transition(|_| Drafted {
      title: title.to_string(),
      body: body.to_string(),
    })
  }
}

#[spec(for = Drafted, with = ReviewerRights)]
impl Document {
  #[transit(to = Reviewed)]
  pub fn review(self, note: Option<&str>) {
    self.transition(|Drafted { title, body }| Reviewed {
      title,
      body,
      reviewer_note: note.map(String::from),
    })
  }
}

#[spec(for = Reviewed)]
impl Document {
  #[transit(to = Published)]
  pub fn publish(self) {
    self.transition(
      |Reviewed {
         title,
         body,
         reviewer_note,
       }| Published {
        title,
        body,
        reviewer_note,
      },
    )
  }
}

// Display impl only for published docs. you can't print a draft by
// accident because the method doesn't even exist on those types :D
impl fmt::Display for Document<Published, Caps<(ReviewerRights,)>> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let published: &Published = self.state.select(DocumentState);
    write!(
      f,
      "---\ntitle: {}\nbody: {}\n",
      published.title, published.body
    )?;
    if let Some(ref note) = published.reviewer_note {
      write!(f, "reviewer note: {note}\n")?;
    }
    write!(f, "---")
  }
}

fn main() {
  let caps = caps!(ReviewerRights);

  let doc = Document::new(Blank, caps)
    .draft("Hello!", "This is a state machine with data.")
    .review(Some("Looks good to me!"))
    .publish();

  println!("{doc}");
}
