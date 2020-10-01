#[allow(unused_imports)]
use super::Typescriptify;

#[cfg(test)]
mod macro_test {
    use super::Typescriptify;
    use quote::quote;

    macro_rules! assert_conversion {
        ($tokens:expr,$expected:expr) => {{
            let declarations = Typescriptify::new($tokens)
                .parse()
                .export_type_definition_source()
                .declarations;

            assert_eq!(declarations, $expected)
        }};
    }

    // The crate only converts complex types, so we need to wrap the type in a struct to check its
    // conversion. We could also write these tests at the conversion logic inside the crate but
    // right now I'm just tring to get as much value for as little effort as possible.
    macro_rules! assert_type_conversion {
        ($rust:ty,$ts:literal) => {
            assert_conversion!(
                struct_with_type!($rust),
                format!("export type Test = {{ t: {} }}", $ts)
            )
        };
    }

    macro_rules! struct_with_type {
        ($type:ty) => {
            quote!(
                struct Test {
                    t: $type,
                }
            );
        };
    }

    macro_rules! assert_converts_to_number {
        ($type:ty) => {
            assert_type_conversion!($type, "number");
        };
    }

    // Untested Conversions:
    //  Path, PathBuf
    //  Box, Cow, Rc, Arc, Cell, RefCell
    //  Duration
    //  SystemTime
    //  VecDeque, LinkedList
    //  BTreeMap, BTreeSet
    //  Fn, FnOnce, FnMut
    //  TraitsObject, ImplTrait
    //  Literally every other type

    #[test]
    fn boolean_type_conversion() {
        assert_type_conversion!(bool, "boolean");
    }

    #[test]
    fn numeric_type_conversions() {
        assert_converts_to_number!(i8);
        assert_converts_to_number!(i16);
        assert_converts_to_number!(i32);
        assert_converts_to_number!(i64);
        assert_converts_to_number!(i128);
        assert_converts_to_number!(isize);

        assert_converts_to_number!(u8);
        assert_converts_to_number!(u16);
        assert_converts_to_number!(u32);
        assert_converts_to_number!(u64);
        assert_converts_to_number!(u128);
        assert_converts_to_number!(usize);

        assert_converts_to_number!(f32);
        assert_converts_to_number!(f64);
    }

    #[test]
    fn string_type_conversions() {
        assert_type_conversion!(char, "string");

        assert_type_conversion!(String, "string");
        assert_type_conversion!(str, "string");
    }

    #[test]
    fn collection_conversion() {
        assert_type_conversion!([String], "string []");
        assert_type_conversion!(Vec<String>, "string []");
        assert_type_conversion!(HashMap<String, String>, "{ [key: string]: string }");
        assert_type_conversion!(HashSet<String>, "string []");
    }

    #[test]
    fn monad_conversion() {
        assert_type_conversion!(Option<String>, "string | null");
        assert_type_conversion!(Result<String, String>, "{ Ok: string } | { Err: string }");
        assert_type_conversion!(Either<String, String>, "{ Left: string } | { Right: string }");
    }

    #[test]
    fn reference_conversion() {
        assert_type_conversion!(&str, "string");
    }

    #[test]
    fn tuple_conversion() {
        assert_type_conversion!((i32, String), "[number , string]")
    }

    #[test]
    fn custom_type_conversion() {
        assert_type_conversion!(MyCustomType, "MyCustomType")
    }

    // This is tested implicitly in other tests but it's ok to be explicit, plus this
    // one tests structs with multiple fields
    #[test]
    fn struct_conversion() {
        let tokens = quote!(
            struct Test {
                i: i32,
                s: String,
            }
        );
        assert_conversion!(tokens, "export type Test = { i: number; s: string }")
    }

    #[test]
    fn simple_enum_is_converted() {
        let tokens = quote!(
            #[serde(tag = "t", content = "c")]
            enum SimpleEnum {
                Foo,
                Bar,
            }
        );
        assert_conversion!(
            tokens,
            "export type SimpleEnum = \n | { t: \"Foo\" } \n | { t: \"Bar\" }"
        )
    }

    #[test]
    fn enum_with_struct_variants_is_converted() {
        let tokens = quote!(
            #[serde(tag = "t", content = "c")]
            enum ComplexEnum {
                Foo(String),
                Bar { Baz: i32 },
            }
        );
        assert_conversion!(
            tokens, "export type ComplexEnum = \n | { t: \"Foo\"; c: string } \n | { t: \"Bar\"; c: { Baz: number } }"
        )
    }

    #[test]
    fn enum_with_complex_inner_types_is_converted() {
        let tokens = quote!(
            #[serde(tag = "t", content = "c")]
            enum ComplexEnum {
                Foo(Vec<Bar>),
                Bar(Option<Baz>),
            }
        );
        assert_conversion!(
            tokens, "export type ComplexEnum = \n | { t: \"Foo\"; c: Bar [] } \n | { t: \"Bar\"; c: Baz | null }"
        )
    }

    // Error tests

    #[test]
    fn conversion_is_only_valid_for_structs_or_enums() {
        let tokens = quote!(type Foo(String));

        let result = std::panic::catch_unwind(move || Typescriptify::new(tokens).parse());
        match result {
            Ok(_) => panic!("expecting panic!"),
            Err(ref msg) => assert!(msg
                .downcast_ref::<String>()
                .unwrap()
                .contains("expected one of: `struct`, `enum`, `union`")),
        }
    }

    // TODO: None of these compiled at all previously, nevermind passing, and it's less bang for buck fixing it than getting the type conversion test coverage.
    // We should probably bring them back in future though.

    // #[test]
    // fn tag_clash_in_enum() {
    //     let tokens = quote!(
    //         #[derive(Serialize)]
    //         #[serde(tag = "kind")]
    //         enum A {
    //             Unit,
    //             B { kind: i32, b: String },
    //         }
    //     );

    //     let result = std::panic::catch_unwind(move || Typescriptify::new(tokens).parse());
    //     match result {
    //         Ok(_x) => assert!(false, "expecting panic!"),
    //         Err(ref msg) => assert_snapshot_matches!( msg.downcast_ref::<String>().unwrap(),
    //         @r###"2 errors:
    // # variant field name `kind` conflicts with internal tag
    // # clash with field in "A::B". Maybe use a #[serde(content="...")] attribute."###
    //         ),
    //     }
    // }
    // #[test]
    // fn flatten_is_fail() {
    //     let tokens = quote!(
    //         #[derive(Serialize)]
    //         struct SSS {
    //             a: i32,
    //             b: f64,
    //             #[serde(flatten)]
    //             c: DDD,
    //         }
    //     );
    //     let result = std::panic::catch_unwind(move || Typescriptify::new(tokens).parse(true));
    //     match result {
    //         Ok(_x) => assert!(false, "expecting panic!"),
    //         Err(ref msg) => assert_snapshot_matches!( msg.downcast_ref::<String>().unwrap(),
    //         @"SSS: #[serde(flatten)] does not work for typescript-definitions."
    //         ),
    //     }
    // }

    // #[test]
    // fn verify_is_recognized() {
    //     let tokens = quote!(
    //         #[derive(Serialize)]
    //         #[ts(guard = "blah")]
    //         struct S {
    //             a: i32,
    //             b: f64,
    //         }
    //     );
    //     let result = std::panic::catch_unwind(move || Typescriptify::new(tokens).parse_verify());
    //     match result {
    //         Ok(_x) => assert!(false, "expecting panic!"),
    //         Err(ref msg) => assert_snapshot_matches!( msg.downcast_ref::<String>().unwrap(),
    //         @r###"S: guard must be true or false not ""blah"""###
    //         ),
    //     }
    // }
    // #[test]
    // fn turbofish() {
    //     let tokens = quote!(
    //         #[derive(TypeScriptify)]
    //         #[ts(turbofish = "<i32>")]
    //         struct S<T> {
    //             a: i32,
    //             b: Vec<T>,
    //         }
    //     );
    //     let ty = Typescriptify::parse(true, tokens);
    //     let i = &ty.ctxt.ident;
    //     let g = ty.ctxt.global_attrs.turbofish.unwrap_or_else(|| quote!());
    //     let res = quote!(#i#g::type_script_ify()).to_string();
    //     assert_snapshot_matches!(res,
    //     @"S < i32 > :: type_script_ify ( )" );
    // }
    // #[test]
    // fn bad_ts_as() {
    //     let tokens = quote!(
    //         #[derive(TypeScriptify)]

    //         struct S<T> {
    //             #[ts(ts_as = "😀i32>")]
    //             a: i32,
    //             #[ts(ts_as = "T[]")]
    //             b: Vec<T>,
    //         }
    //     );
    //     let result = std::panic::catch_unwind(move || Typescriptify::new(tokens).parse(true));
    //     match result {
    //         Ok(_x) => assert!(false, "expecting panic!"),
    //         Err(ref msg) => assert_snapshot_matches!( msg.downcast_ref::<String>().unwrap(),
    //         @r###"2 errors:
    // # ts_as: "😀i32>" is not a valid rust type
    // # ts_as: "T[]" is not a valid rust type"###
    //         ),
    //     }
    // }
}
