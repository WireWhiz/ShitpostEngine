//! `#[task]` — turn a normal function into a constructor for a `TaskDefinition`.
//!
//! Given a plain function:
//!
//! ```ignore
//! #[task]
//! fn add(a: f32, b: f32) -> f32 {
//!     a + b
//! }
//! ```
//!
//! the macro replaces it with a function of the *same name and visibility* that
//! builds the matching `Arc<TaskDefinition>`. The original function is kept
//! intact — just renamed, made private, nested inside the constructor, and
//! marked `#[inline]` — and the callback simply calls it:
//!
//! ```ignore
//! fn add() -> std::sync::Arc<TaskDefinition> {
//!     #[inline]
//!     fn __task_impl(a: f32, b: f32) -> f32 {
//!         a + b
//!     }
//!
//!     std::sync::Arc::new(TaskDefinition {
//!         queue_params: Vec::new(),
//!         data_params: vec![
//!             TaskDataParam { type_def: <f32 as MetaType>::meta_def() },
//!             TaskDataParam { type_def: <f32 as MetaType>::meta_def() },
//!         ],
//!         output: TaskOutput { type_def: <f32 as MetaType>::meta_def() },
//!         callback: |args, queues, output| unsafe {
//!             let __task_result: f32 = __task_impl(
//!                 *<f32 as MetaType>::from_slice(args[0]),
//!                 *<f32 as MetaType>::from_slice(args[1]),
//!             );
//!             std::ptr::write(output.as_mut_ptr() as *mut f32, __task_result);
//!         },
//!     })
//! }
//! ```
//!
//! Because the body lives in a real function, all control flow (`return`, `?`,
//! `loop`, …) behaves exactly as written.
//!
//! These items must be in scope wherever `#[task]` is used: `TaskDefinition`,
//! `TaskDataParam`, `TaskOutput`, and the `MetaType` trait.
//!
//! ## Parameters
//!
//! Every parameter is a **data parameter** (it must implement `MetaType`, and
//! is passed by value into the impl) unless it is tagged `#[queue]`, which
//! marks it as a **queue parameter**. Queue support is recognised but not yet
//! lowered — see [`expand`].

use proc_macro2::Literal;
use quote::quote;
use syn::{FnArg, Ident, ItemFn, Pat, PatType, ReturnType, Type, Visibility, spanned::Spanned};

/// The helper attribute that marks a parameter as a queue parameter.
const QUEUE_ATTR: &str = "queue";

/// The name given to the nested copy of the user's function.
const IMPL_FN_NAME: &str = "__task_impl";

/// Build the replacement function from the annotated `fn`.
pub fn expand(func: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    reject_unsupported(&func)?;

    let (data_params, queue_params) = classify_params(&func)?;

    // Queue parameters are recognised, but the queue runtime types don't exist
    // yet, so we can't lower them. Fail with a pointer to the spot that needs
    // filling in rather than silently mis-treating them as data parameters.
    //
    // To add support later, this is the only place that changes: emit the
    // `queue_params` entries, and pass each queue parameter into `__task_impl`
    // from `queues` (in its original argument position) the way data parameters
    // are passed from `args`.
    if let Some((ident, _)) = queue_params.first() {
        return Err(syn::Error::new(
            ident.span(),
            "`#[queue]` parameters are not supported yet; \
             implement queue lowering in `task_macro::expand` before using them",
        ));
    }

    let output_ty = output_type(&func.sig.output);

    // `data_params: vec![ TaskDataParam { .. }, .. ]`, in declaration order.
    let data_param_defs = data_params.iter().map(|(_, ty)| {
        quote! {
            TaskDataParam { type_def: <#ty as MetaType>::meta_def() }
        }
    });

    // Arguments for the call, in order: deserialise `args[i]` and copy it out
    // by value so it matches the impl's by-value parameter.
    let call_args = data_params.iter().enumerate().map(|(i, (_, ty))| {
        let index = Literal::usize_unsuffixed(i);
        quote! {
            *<#ty as MetaType>::from_slice(args[#index])
        }
    });

    let impl_fn = build_impl_fn(&func);
    let impl_name = Ident::new(IMPL_FN_NAME, func.sig.ident.span());

    let vis = &func.vis;
    let attrs = &func.attrs; // doc comments etc. describe the public constructor
    let name = &func.sig.ident;

    Ok(quote! {
        #(#attrs)*
        #vis fn #name() -> ::std::sync::Arc<TaskDefinition> {
            #impl_fn

            ::std::sync::Arc::new(TaskDefinition {
                // No queue parameters are emitted yet (see `expand`).
                queue_params: ::std::vec::Vec::new(),
                data_params: ::std::vec![ #(#data_param_defs),* ],
                output: TaskOutput {
                    type_def: <#output_ty as MetaType>::meta_def(),
                },
                callback: |args, queues, output| unsafe {
                    let __task_result: #output_ty = #impl_name( #(#call_args),* );
                    ::std::ptr::write(
                        output.as_mut_ptr() as *mut #output_ty,
                        __task_result,
                    );
                },
            })
        }
    })
}

/// The user's function, kept verbatim but renamed, made private, marked
/// `#[inline]`, and with any `#[queue]` markers removed from its parameters.
fn build_impl_fn(func: &ItemFn) -> ItemFn {
    let mut impl_fn = func.clone();

    // The user's outer attributes (docs, etc.) belong on the public constructor,
    // not on this private helper; replace them with `#[inline]`.
    impl_fn.attrs = vec![syn::parse_quote!(#[inline])];
    impl_fn.vis = Visibility::Inherited;
    impl_fn.sig.ident = Ident::new(IMPL_FN_NAME, func.sig.ident.span());

    // `#[queue]` is our inert helper; strip it so the re-emitted fn stays valid.
    for input in &mut impl_fn.sig.inputs {
        if let FnArg::Typed(arg) = input {
            arg.attrs.retain(|attr| !attr.path().is_ident(QUEUE_ATTR));
        }
    }

    impl_fn
}

/// Split parameters into `(data, queue)` lists of `(identifier, type)` pairs.
///
/// A parameter is a queue parameter iff it carries `#[queue]`; everything else
/// is a data parameter and must implement `MetaType`.
type Param = (Ident, Type);
fn classify_params(func: &ItemFn) -> syn::Result<(Vec<Param>, Vec<Param>)> {
    let mut data = Vec::new();
    let mut queues = Vec::new();

    for input in &func.sig.inputs {
        let arg = match input {
            FnArg::Typed(arg) => arg,
            FnArg::Receiver(recv) => {
                return Err(syn::Error::new(
                    recv.span(),
                    "`#[task]` functions cannot take `self`",
                ));
            }
        };

        let param = (param_ident(arg)?, (*arg.ty).clone());
        if is_queue_param(arg) {
            queues.push(param);
        } else {
            data.push(param);
        }
    }

    Ok((data, queues))
}

/// A parameter is a queue parameter when it is tagged with `#[queue]`.
fn is_queue_param(arg: &PatType) -> bool {
    arg.attrs
        .iter()
        .any(|attr| attr.path().is_ident(QUEUE_ATTR))
}

/// Extract the identifier from a plain `name: Type` parameter.
fn param_ident(arg: &PatType) -> syn::Result<Ident> {
    match arg.pat.as_ref() {
        Pat::Ident(pat) => Ok(pat.ident.clone()),
        other => Err(syn::Error::new(
            other.span(),
            "`#[task]` parameters must be plain identifiers, e.g. `value: f32`",
        )),
    }
}

/// The return type as tokens, defaulting to `()` when the `-> T` is omitted.
fn output_type(output: &ReturnType) -> proc_macro2::TokenStream {
    match output {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => quote! { #ty },
    }
}

/// Reject function shapes the macro can't lower, each with a clear message.
fn reject_unsupported(func: &ItemFn) -> syn::Result<()> {
    let sig = &func.sig;

    if sig.asyncness.is_some() {
        return Err(syn::Error::new(
            sig.fn_token.span(),
            "`#[task]` functions cannot be `async`",
        ));
    }
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return Err(syn::Error::new(
            sig.generics.span(),
            "`#[task]` functions cannot be generic",
        ));
    }
    if let Some(variadic) = &sig.variadic {
        return Err(syn::Error::new(
            variadic.span(),
            "`#[task]` functions cannot be variadic",
        ));
    }

    Ok(())
}
