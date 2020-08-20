// Copyright 2019 Ian Castleden
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::patch::nl;
use super::QuoteT;
use super::{filter_visible, ident_from_str, ParseContext, QuoteMaker, QuoteMakerKind};
use crate::patch::tsignore;
use proc_macro2::Literal;
use quote::quote;
use serde_derive_internals::{ast, ast::Variant, attr::TagType};
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
    fn from_enum(e: &'a TagType) -> Self {
        match e {
            TagType::Internal { tag, .. } => TagInfo {
                tag: Some(tag),
                content: None,
                untagged: false,
            },
            TagType::Adjacent { tag, content, .. } => TagInfo {
                tag: Some(tag),
                content: Some(&content),
                untagged: false,
            },
            TagType::External => TagInfo {
                tag: None,
                content: None,
                untagged: false,
            },
            TagType::None => TagInfo {
                tag: None,
                content: None,
                untagged: true,
            },
        }
    }
}

struct VariantQuoteMaker {
    /// message type possibly including tag key value
    pub source: QuoteT,
    /// enum factory quote token stream
    // pub enum_factory: Result<QuoteT, &'static str>,
    /// inner type token stream
    pub inner_type: Option<QuoteT>,
}

#[allow(clippy::or_fun_call)]
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
            .iter()
            .filter(|v| !v.attrs.skip_serializing())
            .collect();

        // is typescript enum compatible
        let is_enum = taginfo.tag.is_none()
            && taginfo.content.is_none()
            && variants.iter().all(|v| matches!(v.style, ast::Style::Unit));

        if is_enum {
            let v = &variants
                .into_iter()
                .map(|v| v.attrs.name().serialize_name()) // use serde name instead of v.ident
                .collect::<Vec<_>>();

            let k = v.iter().map(|v| ident_from_str(&v)).collect::<Vec<_>>();

            return QuoteMaker {
                source: quote! ( { #(#k = #v),* } ),
                enum_factory: Err("factory cannot be created with raw enum type"),
                enum_handler: Err("handler cannot be created with raw enum type"),
                kind: QuoteMakerKind::Enum,
            };
        }

        let content: Vec<(&Variant, VariantQuoteMaker)> = variants
            .iter()
            .map(|variant| {
                (
                    *variant,
                    match variant.style {
                        ast::Style::Struct => self.derive_struct_variant(
                            &taginfo,
                            variant,
                            &variant.fields,
                            ast_container,
                        ),
                        ast::Style::Newtype => {
                            self.derive_newtype_variant(&taginfo, variant, &variant.fields[0])
                        }
                        ast::Style::Tuple => {
                            self.derive_tuple_variant(&taginfo, variant, &variant.fields)
                        }
                        ast::Style::Unit => self.derive_unit_variant(&taginfo, variant),
                    },
                )
            })
            .collect::<Vec<_>>();

        // OK generate A | B | C etc
        let newl = nl();
        let tsignore = tsignore();
        let body = content.iter().map(|(_, q)| q.source.clone());

        let enum_factory = taginfo
            .tag
            .as_ref()
            .ok_or("serde tag must be specified to create enum factory")
            .and_then(|tag_key| -> Result<QuoteT, &'static str> {
                let args = content.iter().map(|(_, q)| {
                    q.inner_type
                        .as_ref()
                        .map(|inner_type| quote!(content: #inner_type))
                        .unwrap_or(quote!())
                });
                let has_args = content.iter().map(|(_, q)| q.inner_type.is_some());
                let ret_constructs = content.iter().zip(has_args).map(
                    |((v, _), has_args): (&(&Variant, VariantQuoteMaker), bool)| {
                        let tag_name_str = Literal::string(&self.variant_name(v));
                        let tag_key_str = Literal::string(tag_key);
                        if has_args {
                            taginfo
                                .content
                                .map(|content_key| {
                                    let content_key_str = Literal::string(content_key);
                                    quote!({ #tag_key_str: #tag_name_str, #content_key_str: content })
                                })
                                .unwrap_or(quote!({ #tag_key: #tag_name_str, ...content }))
                        } else {
                            quote!({ #tag_key_str: #tag_name_str })
                        }
                    },
                );
                let ret_constructs_copy = ret_constructs.clone();
                let tag_name = variants.iter().map(|v| v.ident.clone());
                // let tag_key_dq_1 = Literal::string(tag_key);
                // let ret_type = std::iter::repeat(ret_type_1.clone());

                let newls = std::iter::repeat(quote!(#newl));

                let type_ident_str = super::patch(&self.ident.to_string()).to_string();
                let type_ident_1 = ident_from_str(&type_ident_str);
                let type_ident = std::iter::repeat(type_ident_1.clone());
                let export_factory_ident_1 = ident_from_str(
                    self.global_attrs
                        .ts_factory_name
                        .as_ref()
                        // default naming
                        .unwrap_or(&format!("{}Factory", &type_ident_str)),
                );

                let export_factory_type_ident_1 = ident_from_str(
                    self.global_attrs
                        .ts_factory_return_name
                        .as_ref()
                        // default naming
                        .unwrap_or(&format!("{}ReturnType", &export_factory_ident_1))
                );

                let args_copy = args.clone();
                let args_copy2 = args.clone();
                let tag_name_copy = tag_name.clone();
                let tag_name_copy2 = tag_name.clone();
                let newls_copy = newls.clone();
                let newls_copy2 = newls.clone();
                Ok(
                    quote!(export const #type_ident_1 = Object.freeze({
                        #( #newls_copy2  #tag_name_copy2(#args_copy2): #type_ident {
                            return #ret_constructs_copy
                        },)*#newl
                    });#newl
                    export const #export_factory_ident_1 = <R> (fn: (message: #type_ident_1) => R): #export_factory_type_ident_1<R> => Object.freeze({
                            #( #newls  #tag_name(#args): R {
                                return fn(#ret_constructs)
                            },)*#newl
                        });#newl
                        export type #export_factory_type_ident_1<R = void> = {
                            #( #newls_copy  #tag_name_copy(#args_copy): R;)*#newl
                        };#newl
                    ),
                )
            });

        let enum_handler = taginfo
            .tag
            .as_ref()
            .ok_or("serde tag must be specified to create enum handler")
            .and_then(|tag_key| -> Result<QuoteT, &'static str> {
                let mut conflict_aliases = Vec::new();
                let (args, variant_types): (Vec<_>, Vec<_>) = content.iter().map(|(v, q)|
                    q.inner_type
                    .as_ref()
                    .map(|inner_type| {
                        let variant_ident = &v.ident;
                        if format!("{}", variant_ident) == format!("{}", inner_type) {
                            let variant_inner_type = ident_from_str(&format!("_{}", variant_ident));
                            conflict_aliases.push(quote!(type #variant_inner_type = #inner_type;));
                            (quote!(message: #inner_type), quote!(export type #variant_ident = #variant_inner_type;))
                        } else {
                            (quote!(message: #inner_type), quote!(export type #variant_ident = #inner_type;))
                        }
                    }).unwrap_or(
                        (quote!(), quote!())
                    )).unzip();
                let on_tag_name = variants.iter().map(|v| ident_from_str(&format!("on{}",(self.variant_name(v)))));
                let tag_key_dq_1 = Literal::string(tag_key);
                let ret_type_1 = ident_from_str(
                    self.global_attrs
                        .ts_handler_return
                        .as_ref()
                        // default return type to any
                        .unwrap_or(&String::from("any")),
                );
                let ret_type = std::iter::repeat(ret_type_1.clone());

                let newls = std::iter::repeat(quote!(#newl));
                let newls2 = std::iter::repeat(quote!(#newl));
                let newls3 = std::iter::repeat(quote!(#newl));
                let handle_prefix_dq_1 = Literal::string("on");

                let type_ident = super::patch(&self.ident.to_string()).to_string();
                let export_interface_1 = ident_from_str(
                    self.global_attrs
                    .ts_handler_name
                    .as_ref()
                    // default naming
                    .unwrap_or(&format!("Handle{}", &type_ident)));

                let ident_1 = ident_from_str(&type_ident);
                let apply_ident_1 = ident_from_str(&format!("apply{}", &type_ident));
                let access_input_content_1 = taginfo.content.map(|content_key| quote!(input[#content_key])).unwrap_or(quote!(input));

                // type EnumType_VariantName = { ... };
                // let variant_types = content.iter().map(|(v, q)|
                //     q.inner_type
                //     .as_ref()
                //     .map(|inner_type|{
                //         let variant_name = ident_from_str(&format!("{}_{}", type_ident, v.ident));
                //         quote!(export namespace #variant_name = #inner_type;)
                //     }).unwrap_or(
                //         quote!()
                //     ));

                Ok(quote!(export interface #export_interface_1 {
                        #( #newls  #on_tag_name(#args): #ret_type;)*#newl
                    }#newl
                    #(#newls2 #conflict_aliases)*#newl
                    export namespace #ident_1 {
                        #( #newls3 #variant_types )*#newl
                    }
                    export function #apply_ident_1(handler: #export_interface_1): (input: #ident_1) => #ret_type_1 {#newl
                        #tsignore
                        return input => handler[#handle_prefix_dq_1 + input[#tag_key_dq_1]](#access_input_content_1);#newl
                    }#newl
                ))
            });

        let newls = std::iter::repeat(quote!(#newl));
        QuoteMaker {
            source: quote! ( #( #newls | #body)* ),
            enum_factory,
            enum_handler,
            kind: QuoteMakerKind::Union,
        }
    }

    /// Depends on TagInfo for layout
    fn derive_unit_variant(&self, taginfo: &TagInfo, variant: &Variant) -> VariantQuoteMaker {
        let variant_name = variant.attrs.name().serialize_name(); // use serde name instead of variant.ident

        if taginfo.tag.is_none() {
            return VariantQuoteMaker {
                source: quote!(#variant_name),
                inner_type: None,
            };
        }
        let tag = ident_from_str(taginfo.tag.unwrap());
        VariantQuoteMaker {
            source: quote! (
                { #tag: #variant_name }
            ),
            inner_type: None,
        }
    }

    /// Depends on TagInfo for layout
    /// example variant: `C(u32)`
    fn derive_newtype_variant(
        &self,
        taginfo: &TagInfo,
        variant: &Variant,
        field: &ast::Field<'a>,
    ) -> VariantQuoteMaker {
        if field.attrs.skip_serializing() {
            return self.derive_unit_variant(taginfo, variant);
        };
        let ty = self.field_to_ts(field);
        let variant_name = self.variant_name(variant);

        if taginfo.tag.is_none() {
            if taginfo.untagged {
                return VariantQuoteMaker {
                    source: quote! ( #ty ),
                    inner_type: Some(ty),
                };
            };
            let tag = ident_from_str(&variant_name);

            return VariantQuoteMaker {
                source: quote! (
                    { #tag : #ty }

                ),
                inner_type: Some(ty),
            };
        };
        let tag = ident_from_str(taginfo.tag.unwrap());

        let content = if let Some(content) = taginfo.content {
            ident_from_str(&content)
        } else {
            ident_from_str(CONTENT) // should not get here...
        };

        VariantQuoteMaker {
            source: quote! (
                { #tag: #variant_name; #content: #ty }
            ),
            inner_type: Some(ty),
        }
    }

    /// Depends on TagInfo for layout
    /// `C { a: u32, b: u32 }` => `C: { a: number, b: number }`
    fn derive_struct_variant(
        &self,
        taginfo: &TagInfo,
        variant: &Variant,
        fields: &[ast::Field<'a>],
        ast_container: &ast::Container,
    ) -> VariantQuoteMaker {
        use std::collections::HashSet;
        let fields = filter_visible(fields);
        if fields.is_empty() {
            return self.derive_unit_variant(taginfo, variant);
        }

        self.check_flatten(&fields, ast_container);

        let contents = self.derive_fields(&fields).collect::<Vec<_>>();
        let variant_name = self.variant_name(variant);

        let ty_inner = quote!(#(#contents);*);
        let ty = quote! (
            { #ty_inner }
        );

        if taginfo.tag.is_none() {
            if taginfo.untagged {
                return VariantQuoteMaker {
                    source: quote!(#ty),
                    inner_type: Some(ty),
                };
            };
            let tag = ident_from_str(&variant_name);
            return VariantQuoteMaker {
                source: quote! (
                    { #tag : #ty  }
                ),
                inner_type: Some(ty),
            };
        }
        let tag_str = taginfo.tag.unwrap();
        let tag = ident_from_str(tag_str);

        if let Some(content) = taginfo.content {
            let content = ident_from_str(&content);

            VariantQuoteMaker {
                source: quote! (
                    { #tag: #variant_name; #content: #ty }
                ),
                inner_type: Some(ty),
            }
        } else {
            if let Some(ref cx) = self.ctxt {
                let fnames = fields
                    .iter()
                    .map(|field| field.attrs.name().serialize_name())
                    .collect::<HashSet<_>>();
                if fnames.contains(tag_str) {
                    cx.error_spanned_by(
                        tag_str,
                        format!(
                            "clash with field in \"{}::{}\". \
                         Maybe use a #[serde(content=\"...\")] attribute.",
                            ast_container.ident, variant_name
                        ),
                    );
                }
            };
            // spread together tagged no content
            VariantQuoteMaker {
                source: quote! (
                    { #tag: #variant_name; #ty_inner }
                ),
                inner_type: Some(ty),
            }
        }
    }

    #[inline]
    fn variant_name(&self, variant: &Variant) -> String {
        variant.attrs.name().serialize_name() // use serde name instead of variant.ident
    }

    /// `B(u32, u32)` => `B: [number, number]`
    fn derive_tuple_variant(
        &self,
        taginfo: &TagInfo,
        variant: &Variant,
        fields: &[ast::Field<'a>],
    ) -> VariantQuoteMaker {
        let variant_name = self.variant_name(variant);
        let fields = filter_visible(fields);
        let contents = self.derive_field_tuple(&fields);
        let ty = quote!([ #(#contents),* ]);

        if taginfo.tag.is_none() {
            if taginfo.untagged {
                return VariantQuoteMaker {
                    source: quote! (#ty),
                    inner_type: Some(ty),
                };
            }
            let tag = ident_from_str(&variant_name);
            return VariantQuoteMaker {
                source: quote! ({ #tag : #ty }),
                inner_type: Some(ty),
            };
        };

        let tag = ident_from_str(taginfo.tag.unwrap());
        let content = if let Some(content) = taginfo.content {
            ident_from_str(&content)
        } else {
            ident_from_str(CONTENT)
        };

        VariantQuoteMaker {
            source: quote! (
            { #tag: #variant_name; #content : #ty }
            ),
            inner_type: Some(ty),
        }
    }
}
