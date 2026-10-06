// 0.1.2: new file. The parameters of `metagente.toml` that agents read as `@parameters.name`.

use std::collections::{BTreeMap, BTreeSet};

/// The `[parameters]` section of `metagente.toml`. Empty when there is none.
// 0.1.2: compared by the linker, so an agent file is parsed again when the parameters change
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Params {
    /// Parameters whose value is text.
    pub values: BTreeMap<String, String>,
    /// Names that exist in `[parameters]` but are not text (a number, a list ...).
    pub not_text: BTreeSet<String>,
}
