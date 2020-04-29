// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::patch::{eq, nl};
use super::{filter_visible, ident_from_str, ParseContext, QuoteMaker, QuoteMakerKind};
use proc_macro2::Literal;
use quote::quote;
use serde_derive_internals::{ast, ast::Variant, attr::EnumTag};
const CONTENT: &str = "fields"; // default content tag
                                // const TAG: &'static str = "kind"; // default tag tag
struct TagInfo<'a> {
    /// #[serde(tag = "...")]
    tag: Option<&'a str>,
    /// #[serde(content = "...")]
    content: Option<&'a str>,
    /// flattened without tag `{ "key1": "", "key2": "" }`
    untagged: bool,
}
impl<'a> TagInfo<'a> {
    fn from_enum(e: &'a EnumTag) -> Self {
        match e {
            EnumTag::Internal { tag, .. } => TagInfo {
                tag: Some(tag),
                content: None,
                untagged: false,
            },
            EnumTag::Adjacent { tag, content, .. } => TagInfo {
                tag: Some(tag),
                content: Some(&content),
                untagged: false,
            },
            EnumTag::External => TagInfo {
                tag: None,
                content: None,
                untagged: false,
            },
            EnumTag::None => TagInfo {
                tag: None,
                content: None,
                untagged: true,
            },
        }
    }
}

impl<'a> ParseContext {
    pub(crate) fn derive_enum(
        &self,
        variants: &[ast::Variant<'a>],
        ast_container: &ast::Container,
    ) -> QuoteMaker {
        // https://serde.rs/enum-representations.html
        let taginfo = TagInfo::from_enum(ast_container.attrs.tag());

        // remove skipped ( check for #[serde(skip)] )
        let variants: Vec<&ast::Variant<'a>> = variants
            .into_iter()
            .filter(|v| !v.attrs.skip_serializing())
            .collect();

        // is typescript enum compatible
        let is_enum = variants.iter().all(|v| matches!(v.style, ast::Style::Unit));

        if is_enum {
            let v = &variants
                .into_iter()
                .map(|v| v.attrs.name().serialize_name()) // use serde name instead of v.ident
                .collect::<Vec<_>>();

            let k = v.iter().map(|v| ident_from_str(&v)).collect::<Vec<_>>();
            let verify = if self.gen_guard {
                let obj = &self.arg_name;
                let o = (0..v.len()).map(|_| obj.clone());
                let eq = (0..v.len()).map(|_| eq());

                Some(quote!(
                    {

                        if (!((#(#o #eq #v)||*))) return false;
                        return true;
                    }
                ))
            } else {
                None
            };

            return QuoteMaker {
                source: quote! ( { #(#k = #v),* } ),
                verify,
                kind: QuoteMakerKind::Enum,
            };
        }

        let content = variants
            .iter()
            .map(|variant| match variant.style {
                ast::Style::Struct => {
                    self.derive_struct_variant(&taginfo, variant, &variant.fields, ast_container)
                }
                ast::Style::Newtype => {
                    self.derive_newtype_variant(&taginfo, variant, &variant.fields[0])
                }
                ast::Style::Tuple => self.derive_tuple_variant(&taginfo, variant, &variant.fields),
                ast::Style::Unit => self.derive_unit_variant(&taginfo, variant),
            })
            .collect::<Vec<_>>();

        // OK generate A | B | C etc
        let newl = nl();
        let body = content.iter().map(|q| q.source.clone());
        let nl = content.iter().map(|_| quote!(#newl));
        let verify = if self.gen_guard {
            let v = content.iter().map(|q| q.verify.clone().unwrap());

            let obj = &self.arg_name;
            let nl = content.iter().map(|_| quote!(#newl));
            // obj can't be null or undefined
            Some(quote!(
                {
                    if (#obj == undefined) return false;

                    #( #nl if ( ( () => #v )() ) return true; )*
                    #newl return false;
                }
            ))
        } else {
            None
        };

        QuoteMaker {
            source: quote! ( #( #nl | #body)* ),
            verify,
            kind: QuoteMakerKind::Union {
                enum_factory: None,
                enum_handler: None,
            },
        }
    }
    fn derive_unit_variant(&self, taginfo: &TagInfo, variant: &Variant) -> QuoteMaker {
        let variant_name = variant.attrs.name().serialize_name(); // use serde name instead of variant.ident
        let eq = eq();

        if taginfo.tag.is_none() {
            let verify = if self.gen_guard {
                let obj = &self.arg_name;
                Some(quote!(
                    {
                        return #obj #eq #variant_name;
                    }
                ))
            } else {
                None
            };
            return QuoteMaker {
                source: quote!(#variant_name),
                verify,
                kind: QuoteMakerKind::Object,
            };
        }
        let tag = ident_from_str(taginfo.tag.unwrap());
        let verify = if self.gen_guard {
            let obj = &self.arg_name;
            Some(quote!(
                {
                    return #obj.#tag #eq #variant_name;
                }
            ))
        } else {
            None
        };
        QuoteMaker {
            source: quote! (
                { #tag: #variant_name }
            ),
            verify,
            kind: QuoteMakerKind::Object,
        }
    }

    fn derive_newtype_variant(
        &self,
        taginfo: &TagInfo,
        variant: &Variant,
        field: &ast::Field<'a>,
    ) -> QuoteMaker {
        if field.attrs.skip_serializing() {
            return self.derive_unit_variant(taginfo, variant);
        };
        let ty = self.field_to_ts(field);
        let variant_name = self.variant_name(variant);
        let obj = &self.arg_name;

        if taginfo.tag.is_none() {
            if taginfo.untagged {
                let verify = if self.gen_guard {
                    let v = self.verify_type(&obj, field);

                    Some(quote!( { #v; return true }))
                } else {
                    None
                };
                return QuoteMaker {
                    source: quote! ( #ty ),
                    verify,
                    kind: QuoteMakerKind::Object,
                };
            };
            let tag = ident_from_str(&variant_name);

            let verify = if self.gen_guard {
                let v = quote!(v);
                let verify = self.verify_type(&v, field);
                let eq = eq();
                // #ty might be a Option None and therefore null
                // OTOH #verify might be assuming not null and not undefined
                Some(quote!(
                    {
                        const v = #obj.#tag;
                        if (v #eq undefined) return false;
                        #verify;
                        return true;
                    }
                ))
            } else {
                None
            };
            return QuoteMaker {
                source: quote! (
                    { #tag : #ty }

                ),
                verify,
                kind: QuoteMakerKind::Object,
            };
        };
        let tag = ident_from_str(taginfo.tag.unwrap());

        let content = if let Some(content) = taginfo.content {
            ident_from_str(&content)
        } else {
            ident_from_str(CONTENT) // should not get here...
        };

        let verify = if self.gen_guard {
            let eq = eq();
            let verify = self.verify_type(&quote!(val), field);
            Some(quote!(
            {
                if (!(#obj.#tag #eq #variant_name)) return false;
                const val = #obj.#content;
                if (val #eq undefined) return false;
                #verify;
                return true;
            }))
        } else {
            None
        };
        QuoteMaker {
            source: quote! (
                { #tag: #variant_name; #content: #ty }
            ),
            verify,
            kind: QuoteMakerKind::Object,
        }
    }

    fn derive_struct_variant(
        &self,
        taginfo: &TagInfo,
        variant: &Variant,
        fields: &[ast::Field<'a>],
        ast_container: &ast::Container,
    ) -> QuoteMaker {
        use std::collections::HashSet;
        let fields = filter_visible(fields);
        if fields.is_empty() {
            return self.derive_unit_variant(taginfo, variant);
        }

        self.check_flatten(&fields, ast_container);

        let contents = self.derive_fields(&fields).collect::<Vec<_>>();
        let variant_name = self.variant_name(variant);

        let last = nl();
        let nl = contents.iter().map(|_| quote!(#last));
        if taginfo.tag.is_none() {
            if taginfo.untagged {
                let verify = if self.gen_guard {
                    let verify = self.verify_fields(&self.arg_name, &fields);

                    Some(quote!(
                        {
                            #( #nl #verify;)*
                            #last return true;
                        }
                    ))
                } else {
                    None
                };
                return QuoteMaker {
                    source: quote! (
                        { #(#contents);* }
                    ),
                    verify,
                    kind: QuoteMakerKind::Object,
                };
            };
            let v = &quote!(v);
            let tag = ident_from_str(&variant_name);
            let verify = if self.gen_guard {
                let obj = &self.arg_name;
                let verify = self.verify_fields(&v, &fields);
                Some(quote!(
                    {
                        const v = #obj.#tag;
                        if (v == undefined) return false;
                        #(#nl #verify;)*
                        #last return true;
                    }
                ))
            } else {
                None
            };
            return QuoteMaker {
                source: quote! (
                    { #tag : { #(#contents);* }  }
                ),
                verify,
                kind: QuoteMakerKind::Object,
            };
        }
        let tag_str = taginfo.tag.unwrap();
        let tag = ident_from_str(tag_str);

        if let Some(content) = taginfo.content {
            let content = ident_from_str(&content);

            let verify = if self.gen_guard {
                let obj = &self.arg_name;
                let v = quote!(v);
                let verify = self.verify_fields(&v, &fields);
                let eq = eq();
                Some(quote!(
                {
                    if (!(#obj.#tag #eq #variant_name)) return false;
                    const v = #obj.#content;
                    if (v == undefined) return false;
                    #(#nl #verify;)*
                    #last return true;
                }
                ))
            } else {
                None
            };
            QuoteMaker {
                source: quote! (
                    { #tag: #variant_name; #content: { #(#contents);* } }

                ),
                verify,
                kind: QuoteMakerKind::Object,
            }
        } else {
            if let Some(ref cx) = self.ctxt {
                let fnames = fields
                    .iter()
                    .map(|field| field.attrs.name().serialize_name())
                    .collect::<HashSet<_>>();
                if fnames.contains(tag_str) {
                    cx.error(format!(
                        "clash with field in \"{}::{}\". \
                         Maybe use a #[serde(content=\"...\")] attribute.",
                        ast_container.ident, variant_name
                    ));
                }
            };
            let verify = if self.gen_guard {
                let obj = &self.arg_name;
                let verify = self.verify_fields(&obj, &fields);
                let eq = eq();
                Some(quote!(
                {
                    if (!(#obj.#tag #eq #variant_name)) return false;
                    #(#nl #verify;)*
                    #last return true;
                }
                ))
            } else {
                None
            };
            QuoteMaker {
                source: quote! (
                    { #tag: #variant_name; #(#contents);* }
                ),
                verify,
                kind: QuoteMakerKind::Object,
            }
        }
    }

    #[inline]
    fn variant_name(&self, variant: &Variant) -> String {
        variant.attrs.name().serialize_name() // use serde name instead of variant.ident
    }

    fn derive_tuple_variant(
        &self,
        taginfo: &TagInfo,
        variant: &Variant,
        fields: &[ast::Field<'a>],
    ) -> QuoteMaker {
        let variant_name = self.variant_name(variant);
        let fields = filter_visible(fields);
        let contents = self.derive_field_tuple(&fields);

        if taginfo.tag.is_none() {
            if taginfo.untagged {
                let verify = if self.gen_guard {
                    let obj = &self.arg_name;
                    let verify = self.verify_field_tuple(&obj, &fields);
                    let eq = eq();
                    let len = Literal::usize_unsuffixed(fields.len());

                    Some(quote!({
                        if (!Array.isArray(#obj) || !(#obj.length #eq #len)) return false;
                        #(#verify;)*
                        return true;
                    }))
                } else {
                    None
                };
                return QuoteMaker {
                    source: quote! (
                     [ #(#contents),* ]
                    ),
                    verify,
                    kind: QuoteMakerKind::Object,
                };
            }
            let tag = ident_from_str(&variant_name);
            let verify = if self.gen_guard {
                let obj = &self.arg_name;
                let v = quote!(v);
                let verify = self.verify_field_tuple(&v, &fields);
                let len = Literal::usize_unsuffixed(fields.len());
                let eq = eq();
                Some(quote!({
                    const v = #obj.#tag;
                    if (!Array.isArray(v) || !(v.length #eq #len)) return false;
                    #(#verify;)*
                    return true;
                }))
            } else {
                None
            };
            return QuoteMaker {
                source: quote! (
                 { #tag : [ #(#contents),* ] }

                ),
                verify,
                kind: QuoteMakerKind::Object,
            };
        };

        let tag = ident_from_str(taginfo.tag.unwrap());
        let content = if let Some(content) = taginfo.content {
            ident_from_str(&content)
        } else {
            ident_from_str(CONTENT)
        };

        let verify = if self.gen_guard {
            let eq = eq();
            let obj = &self.arg_name;
            let v = quote!(v);
            let verify = self.verify_field_tuple(&v, &fields);
            let len = Literal::usize_unsuffixed(fields.len());
            Some(quote!({
                if (!(#obj.#tag #eq #variant_name)) return false;
                const v = #obj.#content;
                if (!Array.isArray(v) || !(v.length #eq #len)) return false;
                #(#verify;)*
                return true;
            }))
        } else {
            None
        };
        QuoteMaker {
            source: quote! (
            { #tag: #variant_name; #content : [ #(#contents),* ] }
            ),
            verify,
            kind: QuoteMakerKind::Object,
        }
    }
}
