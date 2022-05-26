use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct NewMeeting<'r> {
    pub name: Cow<'r, str>,
}
