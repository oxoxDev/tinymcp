//! What a server wants before it will talk, as `DetectAuth` reports it.
//!
//! These lived in the module crate until a host needed to name them: a host
//! that reaches the module over the bus decodes `DetectAuth`'s reply, and
//! without the type it would have to keep a hand-written mirror that nothing
//! checks against the module's own. They carry no behavior beyond three
//! constructors, so they sit here with the rest of the vocabulary.

mod types;

pub use types::{AuthDetection, AuthKind};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
