// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! # Patch
//!
//! we are generating *typescript* from rust tokens so
//! the final result when rendered to a string has a typescript
//! formatting problem. This mod just applies a few patches
//! to make the final result a little more acceptable.
//!

use proc_macro2::Literal;
// use regex::{Captures, Regex};
use std::borrow::Cow;

// In typescript '===' is a single token whereas
// for rust this would be two tokens '==' and '=',
// and fails to generate correct typescript/javascript.
// So we subsitute the operator with this identifier and then patch
// it back *after* we generate the string.
// The problem is that someone, somewhere might have
// an identifer that is this... We hope and pray.
//
// This is also the reason we prefer !(x === y) to x !== y ..
// too much patching.

// no field names have anything but ascii at the moment.

const TRIPPLE_EQ: &str = "\"__============__\"";
const NL_PATCH: &str = "\"__nlnlnlnl__\"";
const PURE_PATCH: &str = "\"__pure__\"";
const TS_IGNORE_PATCH: &str = "\"__ts_ignore__\"";
/*
// type N = [(&'static str, &'static str); 10];
const NAMES: [(&str, &str); 16] = [
    ("brack", r"\s*\[\s+\]"),
    ("brace", r"\{\s+\}"),
    ("colon", r"\s+[:]\s"),
    ("enl", r"\n+\}"),
    ("fnl", r"\{\n+"),
    ("te", TRIPPLE_EQ), // for ===
    ("lt", r"\s<\s"),
    ("gt", r"\s>(\s|$)"),
    ("semi", r"\s+;"),
    ("call", r"\s\(\s+\)\s"),
    ("dot", r"\s\.\s"),
    ("nlpatch", NL_PATCH),         // for adding newlines to output string
    ("tsignore", TS_IGNORE_PATCH), // for adding ts-ignore comments to output string
    ("pure", PURE_PATCH),          // for adding ts-ignore comments to output str"doc", ing
    // ("doc", r#"#\s*\[\s*doc\s*=\s*"(?P<comment>.*?)"\]"#), // for fixing mishandled ts doc comments
    ("nl", r"\n+"),                // last!
];
*/
static PATCHES: [(&str, &str); 4] = [
    (TRIPPLE_EQ, "==="),
    (NL_PATCH, "\n"),                    // for adding newlines to output string
    (TS_IGNORE_PATCH, "//@ts-ignore\n"), // for adding ts-ignore comments to output string
    (PURE_PATCH, "/*#__PURE__*/"),       // for adding ts-ignore comments to output str"doc", ing
                                         // ("doc", r#"#\s*\[\s*doc\s*=\s*"(?P<comment>.*?)"\]"#), // for fixing mishandled ts doc comments
                                         // ("nl", r"\n+"),                // last!
];

// TODO: where does the newline come from? why the double spaces?
// maybe use RegexSet::new(&[.....])
pub fn patch(s: &str) -> Cow<'_, str> {
    let mut working = s.to_string();

    for (from, to) in PATCHES {
        working = working.replace(from, to);
    }

    Cow::Owned(working)
    // RE.replace_all(s, |c: &Captures| {
    //     let key = c.key();
    //     let m = match key {
    //         "brace" => "{}",
    //         "brack" => "[]",
    //         "colon" => ": ",
    //         "fnl" => "{ ",
    //         // "bar" => "\n  | {",
    //         "enl" => " }",
    //         "nl" => " ",
    //         // "result" => "|",
    //         "te" => "===",
    //         "lt" => "<",
    //         "gt" => ">",
    //         "semi" => ";",
    //         "dot" => ".",
    //         "call" => " () ",
    //         "nlpatch" => "\n",
    //         "tsignore" => "//@ts-ignore\n",
    //         "pure" => "/*#__PURE__*/",
    //         "doc" => {
    //             return c.name("comment").map_or(Cow::Borrowed(""), |m| {
    //                 (String::from("\n    /**") + &unescape(&m.as_str()) + "*/\n").into()
    //             })
    //         }
    //         _ => return Cow::Owned(c.get(0).unwrap().as_str().to_owned()), // maybe should just panic?
    //     };
    //     Cow::Borrowed(m)
    // })
}

// lazy_static! {
//     static ref UNESCAPE: Regex = Regex::new(r"\\(.)").unwrap();
// }
/*
struct Unescape<'a> {
    last_char_was_escape: bool,
    chars_left: Chars<'a>,
}

fn unescape_iter<'a>(chars: Chars<'a>) -> Unescape<'a> {
    Unescape {
        last_char_was_escape: false,
        chars_left: chars,
    }
}

impl <'a> Iterator for Unescape<'a> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.last_char_was_escape {
            self.last_char_was_escape = false;
            Some(match self.chars_left.next() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some(other) => other,
                None => '\\',
            })
        } else {
            match self.chars_left.next() {
                Some('\\') => {
                    self.last_char_was_escape = true;
                    self.next()
                }
                other => other,
            }
        }
    }
}

// when we get the string, e.g. newlines, backslashes and quotes are escaped
// used for doc comments!
fn unescape(input: &str) -> Cow<'_, str> {
    Cow::Owned(unescape_iter(input.chars()).collect())
}
*/
#[inline]
pub fn nl() -> Literal {
    Literal::string(&NL_PATCH[1..NL_PATCH.len() - 1])
}

// #[inline]
// pub fn pure() -> Literal {
//     Literal::string(&PURE_PATCH[1..PURE_PATCH.len() - 1])
// }

#[inline]
pub fn tsignore() -> Literal {
    Literal::string(&TS_IGNORE_PATCH[1..TS_IGNORE_PATCH.len() - 1])
}

// #[inline]
// pub fn vbar() -> Ident {
//     ident_from_str(RESULT_BAR)
// }
