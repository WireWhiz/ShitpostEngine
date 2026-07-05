use proc_macro::TokenStream;
use syn::ItemFn;

mod task_def;
#[proc_macro_attribute]
pub fn task(_args: TokenStream, item: TokenStream) -> TokenStream {
    let func = syn::parse_macro_input!(item as ItemFn);
    task_def::expand(func)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

mod meta_type;
#[proc_macro_derive(MetaType, attributes(opaque))]
pub fn derive_meta_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    meta_type::derive_meta_type(input).into()
}
