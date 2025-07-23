use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Attribute, Ident, MetaNameValue, Token, Type, Visibility,
};

/// Top-level input: either
///   `extobj!(struct Name);`
///   or
///   `extobj!(impl Name { vis id: ty, ... });`
/// plus the optional attribute `#[extobj(crate = "...")]`
struct Input {
    attrs: Vec<Attribute>,   // holds the #[extobj(...)] attribute if any
    kw_struct: Option<Span>, // span of the `struct` token if present
    name: Ident,
    fields: Vec<(Visibility, Ident, Type)>,
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        // Parse optional helper attribute(s) first
        let attrs = input.call(Attribute::parse_outer)?;

        let lookahead = input.lookahead1();
        if lookahead.peek(Token![struct]) {
            let kw_struct: Token![struct] = input.parse()?;
            let name = input.parse()?;
            Ok(Input {
                attrs,
                kw_struct: Some(kw_struct.span),
                name,
                fields: Vec::new(),
            })
        } else {
            let _: Token![impl] = input.parse()?;
            let name = input.parse()?;
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
            Ok(Input {
                attrs,
                kw_struct: None,
                name,
                fields,
            })
        }
    }
}

/// Extract the path specified in `#[extobj(crate = "...")]`,
/// defaulting to `::extobj`.
fn crate_path(attrs: &[Attribute]) -> syn::Result<Ident> {
    for attr in attrs {
        if attr.path().is_ident("extobj") {
            let nv: MetaNameValue = attr.parse_args()?;
            if nv.path.is_ident("crate") {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(l),
                    ..
                }) = &nv.value
                {
                    return Ok(format_ident!("{}", l.value()));
                }
            }
            return Err(syn::Error::new_spanned(
                attr,
                r#"expected #[extobj(crate = "...")]"#,
            ));
        }
    }
    Ok(format_ident!("extobj"))
}

/// # Example
/// ```
/// // Default crate name
/// extobj!(struct MyObj);
/// extobj!(impl MyObj { pub value: i32 });
/// ```
///
/// # Example reexport the extobj crate.
/// ```
/// // Custom crate name
/// #[extobj(crate = my_renamed)]
/// extobj!(struct OtherObj);
/// #[extobj(crate = my_renamed)]
/// extobj!(impl OtherObj { pub flag: bool });
/// ```
#[proc_macro]
pub fn extobj(input: TokenStream) -> TokenStream {
    let Input {
        attrs,
        kw_struct,
        name,
        fields,
    } = parse_macro_input!(input as Input);

    // Determine the crate name to use in the generated code
    let extobj = match crate_path(&attrs) {
        Ok(p) => p,
        Err(e) => return e.into_compile_error().into(),
    };

    if kw_struct.is_some() {
        // `extobj!(struct Name);`
        quote! {
            #[derive(Copy, Clone)]
            pub struct #name;

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
                    #extobj::Var::<#name, #ty>::__new()
                };
            }
        });
        quote! { #( #vars )* }
    }
    .into()
}
