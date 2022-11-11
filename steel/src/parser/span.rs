use core::ops::Range;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{list, rvals::FromSteelVal, rvals::IntoSteelVal};

use crate::rvals::SteelVal;

use super::parser::SourceId;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub struct Span {
    start: usize,
    end: usize,
    source_id: Option<SourceId>,
}

impl IntoSteelVal for Span {
    fn into_steelval(self) -> crate::rvals::Result<crate::SteelVal> {
        Ok(list![self.start, self.end])
    }
}

impl FromSteelVal for Span {
    fn from_steelval<'a>(val: &'a crate::SteelVal) -> crate::rvals::Result<Self> {
        if let SteelVal::ListV(l) = val {
            if l.len() != 2 {
                stop!(ConversionError => "cannot convert to a span object: {}", val);
            }

            Ok(Span {
                start: usize::from_steelval(l.get(0).unwrap())?,
                end: usize::from_steelval(l.get(1).unwrap())?,
                source_id: Some(SourceId(usize::from_steelval(l.get(2).unwrap())?)),
            })
        } else {
            stop!(ConversionError => "cannot convert to a span object: {}", val)
        }
    }
}

impl Span {
    #[inline]
    pub const fn new(start: usize, end: usize, source_id: Option<SourceId>) -> Self {
        Self {
            start,
            end,
            source_id,
        }
    }

    #[inline]
    pub const fn double(span: usize, source_id: Option<SourceId>) -> Self {
        Self {
            start: span,
            end: span,
            source_id,
        }
    }

    #[inline]
    pub const fn start(&self) -> usize {
        self.start
    }

    #[inline]
    pub const fn end(&self) -> usize {
        self.end
    }

    #[inline]
    pub const fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    #[inline]
    pub const fn source_id(&self) -> Option<SourceId> {
        self.source_id
    }

    #[inline]
    pub const fn merge(start: Self, end: Self) -> Span {
        // TODO: If this doesn't seem to make sense with macros, we can revisit
        Self::new(start.start, end.end, start.source_id)
    }

    #[inline]
    pub const fn width(&self) -> usize {
        self.end - self.start
    }

    pub fn coalesce_span(spans: &[Span]) -> Span {
        let span = spans.get(0);
        if let Some(span) = span {
            let mut span = *span;
            for s in spans {
                if s.start() < span.start() {
                    span = Span::new(s.start(), span.end(), s.source_id);
                }
                if s.end() > span.end() {
                    span = Span::new(s.start(), s.end(), s.source_id);
                }
            }
            span
        } else {
            Span::new(0, 0, None)
        }
    }
}

impl fmt::Debug for Span {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

// impl From<Range<usize>> for Span {
//     #[inline]
//     fn from(range: Range<usize>) -> Self {
//         Self {
//             start: range.start,
//             end: range.end,
//         }
//     }
// }

impl Into<Range<usize>> for Span {
    #[inline]
    fn into(self) -> Range<usize> {
        self.start..self.end
    }
}

// impl From<(usize, usize)> for Span {
//     #[inline]
//     fn from(range: (usize, usize)) -> Self {
//         Self {
//             start: range.0,
//             end: range.1,
//         }
//     }
// }

impl Into<(usize, usize)> for Span {
    #[inline]
    fn into(self) -> (usize, usize) {
        (self.start, self.end)
    }
}

// impl From<[usize; 2]> for Span {
//     #[inline]
//     fn from(range: [usize; 2]) -> Self {
//         Self {
//             start: range[0],
//             end: range[1],
//         }
//     }
// }

impl Into<[usize; 2]> for Span {
    #[inline]
    fn into(self) -> [usize; 2] {
        [self.start, self.end]
    }
}
