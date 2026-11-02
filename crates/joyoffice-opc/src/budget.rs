//! Resource budgets for untrusted packages (QUALITY-005, ARCHITECTURE §5).
//!
//! Each limit is checked independently and before the work it bounds: entry
//! counts and declared sizes before any inflation, actual sizes during
//! inflation, XML depth and node counts while parsing. A small archive is
//! not assumed to be safe. Callers may raise limits only on an explicit user
//! decision; nothing here relaxes them silently.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    pub max_entries: u64,
    /// Largest uncompressed size of one part.
    pub max_part_bytes: u64,
    /// Largest sum of declared uncompressed sizes.
    pub max_total_bytes: u64,
    /// Largest uncompressed : compressed ratio for parts above
    /// `ratio_floor_bytes`; tiny parts compress arbitrarily well.
    pub max_ratio: u64,
    pub ratio_floor_bytes: u64,
    pub max_xml_depth: usize,
    /// Largest number of XML events (elements, text, comments…) per part.
    pub max_xml_nodes: u64,
    pub max_relationships: usize,
}

impl Default for Budget {
    /// Initial defaults, sized for 1000-page documents with images; tuned by
    /// the QUALITY-006 corpus, not by measurement yet.
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            max_part_bytes: 512 << 20,
            max_total_bytes: 2 << 30,
            max_ratio: 200,
            ratio_floor_bytes: 1 << 20,
            max_xml_depth: 256,
            max_xml_nodes: 50_000_000,
            max_relationships: 100_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BudgetError {
    Entries {
        count: u64,
        limit: u64,
    },
    PartSize {
        part: String,
        size: u64,
        limit: u64,
    },
    TotalSize {
        size: u64,
        limit: u64,
    },
    Ratio {
        part: String,
        ratio: u64,
        limit: u64,
    },
    XmlDepth {
        part: String,
        limit: usize,
    },
    XmlNodes {
        part: String,
        limit: u64,
    },
    Relationships {
        part: String,
        limit: usize,
    },
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BudgetError::Entries { count, limit } => {
                write!(f, "{count} archive items exceed the limit of {limit}")
            }
            BudgetError::PartSize { part, size, limit } => {
                write!(f, "{part}: {size} bytes exceed the part limit of {limit}")
            }
            BudgetError::TotalSize { size, limit } => {
                write!(f, "{size} bytes in total exceed the limit of {limit}")
            }
            BudgetError::Ratio { part, ratio, limit } => {
                write!(f, "{part}: compression ratio {ratio} exceeds {limit}")
            }
            BudgetError::XmlDepth { part, limit } => {
                write!(f, "{part}: XML nested deeper than {limit}")
            }
            BudgetError::XmlNodes { part, limit } => {
                write!(f, "{part}: more than {limit} XML nodes")
            }
            BudgetError::Relationships { part, limit } => {
                write!(f, "{part}: more than {limit} relationships")
            }
        }
    }
}

impl Budget {
    pub(crate) fn check_entries(&self, count: u64) -> Result<(), BudgetError> {
        if count > self.max_entries {
            return Err(BudgetError::Entries {
                count,
                limit: self.max_entries,
            });
        }
        Ok(())
    }

    pub(crate) fn check_part(
        &self,
        part: &str,
        compressed: u64,
        uncompressed: u64,
    ) -> Result<(), BudgetError> {
        if uncompressed > self.max_part_bytes {
            return Err(BudgetError::PartSize {
                part: part.to_owned(),
                size: uncompressed,
                limit: self.max_part_bytes,
            });
        }
        if uncompressed > self.ratio_floor_bytes {
            let ratio = uncompressed / compressed.max(1);
            if ratio > self.max_ratio {
                return Err(BudgetError::Ratio {
                    part: part.to_owned(),
                    ratio,
                    limit: self.max_ratio,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn check_total(&self, total: u64) -> Result<(), BudgetError> {
        if total > self.max_total_bytes {
            return Err(BudgetError::TotalSize {
                size: total,
                limit: self.max_total_bytes,
            });
        }
        Ok(())
    }
}
