use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Expr, Ident, Path, Token, Type, Visibility,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Top-level input: either
///   `extobj!(struct Name);`
///   or
///   `extobj!(impl Name { vis id: ty, ... });`
/// plus the optional attribute `, crate_path = crate::path2` at the end.
struct Input {
    kw_struct: Option<Span>, // span of the `struct` token if present
    name: Name,
    fields: Vec<(Visibility, Ident, Type)>,
    vis: Visibility,
    crate_path: Path,
    init: Option<Expr>,
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        // Parse optional helper attribute(s) first
        let vis: Visibility = input.parse()?;

        if input.peek(Token![struct]) {
            let kw_struct: Token![struct] = input.parse()?;
            let name = Name::Struct(input.parse()?);
            let CratePathArg { path: crate_path } = input.parse()?;

            Ok(Input {
                kw_struct: Some(kw_struct.span),
                name,
                fields: Vec::new(),
                vis,
                crate_path,
                init: None,
            })
        } else {
            let _: Token![impl] = input.parse()?;
            let name = Name::Impl(input.parse()?);
            let content;
            let _brace = syn::braced!(content in input);
            let mut fields = Vec::new();

            while !content.is_empty() {
                let vis: Visibility = content.parse()?;
                let id: Ident = content.parse()?;
                let _: Token![:] = content.parse()?;
                let ty: Type = content.parse()?;
                let _: Option<Token![,]> = content.parse()?;
                fields.push((vis, id, ty));
            }

            let ImplTrailingArgs { crate_path, init } = input.parse()?;

            Ok(Input {
                kw_struct: None,
                name,
                fields,
                vis: Visibility::Inherited,
                init,
                crate_path: crate_path.unwrap_or_else(|| CratePathArg::default().path),
            })
        }
    }
}

enum Name {
    Struct(Ident), // after `struct`
    Impl(Type),    // after `impl`
}

struct CratePathArg {
    path: Path,
}

impl Parse for CratePathArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        // optional trailing comma
        let _: Option<Token![,]> = input.parse()?;

        if input.is_empty() {
            return Ok(CratePathArg::default());
        }

        // expect `crate_path = "ident"`
        let ident: Ident = input.parse()?; // crate_path

        if ident != "crate_path" {
            return Err(syn::Error::new(
                ident.span(),
                "expected `crate_path = <ident>`",
            ));
        }

        let _: Token![=] = input.parse()?;
        let path = input.parse()?;

        Ok(CratePathArg { path })
    }
}

impl Default for CratePathArg {
    fn default() -> Self {
        Self {
            path: Ident::new("extobj", proc_macro2::Span::call_site()).into(),
        }
    }
}

#[derive(Default)]
struct ImplTrailingArgs {
    crate_path: Option<Path>,
    init: Option<Expr>,
}

impl Parse for ImplTrailingArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut args = ImplTrailingArgs::default();

        // allow trailing comma(s) before any keyword
        while input.peek(Token![,]) {
            let _: Token![,] = input.parse()?;
        }

        // parse zero, one or both of:
        //   crate_path = <path>
        //   init       = <expr>
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            if key == "crate_path" {
                if args.crate_path.is_some() {
                    return Err(syn::Error::new(key.span(), "duplicate `crate_path`"));
                }
                let _: Token![=] = input.parse()?;
                args.crate_path = Some(input.parse()?);
            } else if key == "init" {
                if args.init.is_some() {
                    return Err(syn::Error::new(key.span(), "duplicate `init`"));
                }
                let _: Token![=] = input.parse()?;
                args.init = Some(input.parse()?);
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `crate_path = ...` or `init = ...`",
                ));
            }

            // optional comma after each pair
            let _: Option<Token![,]> = input.parse()?;
        }

        Ok(args)
    }
}

/// # Example
/// ```ignore
/// // Default crate name
/// extobj!(struct MyObj);
/// extobj!(impl MyObj { pub value: i32 });
/// ```
///
/// # Example reexport the extobj crate.
/// ```ignore
/// // Custom crate name
/// extobj!(pub struct OtherObj, crate_path = my_renamed);
/// extobj!(impl OtherObj { pub flag: bool }, crate_path = my_renamed);
/// ```
#[proc_macro]
pub fn extobj(input: TokenStream) -> TokenStream {
    let Input {
        kw_struct,
        name,
        fields,
        vis,
        crate_path,
        init,
    } = parse_macro_input!(input as Input);

    let extobj = crate_path;

    let name = match name {
        Name::Struct(ident) => quote!(#ident),
        Name::Impl(ty) => quote!(#ty),
    };

    let init = init.unwrap_or_else(|| syn::parse_quote!({}));

    if kw_struct.is_some() {
        // `extobj!(struct Name);`
        quote! {
            #[derive(Copy, Clone)]
            #vis struct #name;

            impl #extobj::__ExtObjDef for #name {
                #[inline(always)]
                fn defs() -> &'static #extobj::Defs {
                    static DEFS: #extobj::Defs = ::std::sync::RwLock::new(::std::vec::Vec::new());
                    &DEFS
                }
            }
        }
    } else {
        // `extobj!(impl Name { vis id: ty, ... })`
        let vars = fields.into_iter().map(|(vis, id, ty)| {
            quote! {
                #[allow(non_upper_case_globals)]
                #[#extobj::ctor::ctor(crate_path = #extobj::ctor)]
                #vis static #id: #extobj::Var<#name, #ty> = {
                    #init;
                    #extobj::Var::<#name, #ty>::__new()
                };
            }
        });
        quote! { #( #vars )* }
    }
    .into()
}
