use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{Data, DeriveInput, Field, Fields, Index, Member, Visibility, spanned::Spanned};

const OPAQUE_ATTR: &str = "opaque";

pub fn derive_meta_type(input: DeriveInput) -> TokenStream {
    expand(input).unwrap_or_else(syn::Error::into_compile_error)
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let data = {
        let input: &DeriveInput = &input;
        match &input.data {
            Data::Struct(data) => Ok(data),
            Data::Enum(data) => Err(syn::Error::new_spanned(
                data.enum_token,
                "`#[derive(MetaType)]` can only be used on structs, not enums",
            )),
            Data::Union(data) => Err(syn::Error::new_spanned(
                data.union_token,
                "`#[derive(MetaType)]` can only be used on structs, not unions",
            )),
        }
    }?;
    let raw_fields: Vec<(Member, &Field)> = match &data.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|f| (Member::Named(f.ident.clone().unwrap()), f))
            .collect(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, f)| {
                (
                    Member::Unnamed(Index {
                        index: i as u32,
                        span: f.span(),
                    }),
                    f,
                )
            })
            .collect(),
        Fields::Unit => Vec::new(),
    };

    let mut exposed = Vec::with_capacity(raw_fields.len());
    for (member, field) in raw_fields {
        let attrs: &[syn::Attribute] = &field.attrs;
        let mut opaque = false;
        for attr in attrs {
            if !attr.path().is_ident(OPAQUE_ATTR) {
                continue;
            }
            match &attr.meta {
                syn::Meta::Path(_) => opaque = true,
                _ => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "`#[opaque]` does not take any arguments; write it as a bare `#[opaque]`",
                    ));
                }
            }
        }
        if opaque {
            continue;
        }
        let vis: &Visibility = &field.vis;
        if !matches!(vis, Visibility::Public(_)) {
            continue;
        }
        exposed.push((member, field));
    }
    let fields = exposed;

    let struct_ident = &input.ident;
    let mut generics = input.generics.clone();
    for param in generics.type_params_mut() {
        param.bounds.push(syn::parse_quote!(MetaType));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let self_ty = quote!(#struct_ident #ty_generics);

    let field_defs: Vec<TokenStream> = fields
        .into_iter()
        .map(|(member, field)| {
            let self_ty: &TokenStream = &self_ty;
            let member: &Member = &member;
            let field: &Field = field;
            let ty = &field.ty;
            let name = match member {
                Member::Named(ident) => ident.to_string(),
                Member::Unnamed(index) => index.index.to_string(),
            };
            quote_spanned! {field.span()=>
                MetaFieldDefinition {
                    ident: #name,
                    byte_offset: ::std::mem::offset_of!(#self_ty, #member),
                    def: <#ty as MetaType>::meta_def(),
                }
            }
        })
        .collect();

    Ok(quote! {
        impl #impl_generics MetaType for #struct_ident #ty_generics #where_clause {
            fn meta_id() -> MetaTypeId {
                MetaTypeId(global_unique_usize!())
            }

            fn meta_def() -> &'static MetaTypeDefinition {
                static DEF: ::std::sync::OnceLock<MetaTypeDefinition> =
                    ::std::sync::OnceLock::new();
                DEF.get_or_init(|| MetaTypeDefinition {
                    id: Self::meta_id(),
                    ident: ::std::any::type_name::<Self>(),
                    byte_size: ::std::mem::size_of::<Self>(),
                    fields: ::std::vec![ #(#field_defs),* ].leak(),
                    drop: make_drop_fn::<Self>(),
                })
            }
        }
    })
}
