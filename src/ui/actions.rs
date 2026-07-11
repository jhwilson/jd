use std::{fmt, path::PathBuf};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FinalAction {
    Cd(PathBuf),
    Edit(PathBuf),
    Open(String),
}
impl fmt::Display for FinalAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cd(p) => write!(f, "cd\t{}", p.display()),
            Self::Edit(p) => write!(f, "edit\t{}", p.display()),
            Self::Open(u) => write!(f, "open\t{}", u),
        }
    }
}
