use std::{fs, path};

use proc_macro::TokenStream;
use quote::quote;

use derive_syn_parse::Parse;
use syn::{Ident, LitStr, parse_macro_input};

#[derive(Parse)]
struct RegisterAllModulesMacroArgs {
    modules_path: LitStr,
}

fn visit_dirs(dir: &path::Path, callback: &mut impl FnMut(&fs::DirEntry)) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, callback); // Recursion
            } else {
                callback(&entry);
            }
        }
    } else {
        eprintln!("Failed to read dir {}", dir.to_str().unwrap());
    }
}

#[proc_macro]
pub fn register_all_modules(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as RegisterAllModulesMacroArgs);
    let root_dir = path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    let path_lit = args.modules_path.token().to_string();
    let path_content = path_lit.trim_matches('"');
    let modules_dir = root_dir.join(path_content);
    let modules_dir_lit = LitStr::new(modules_dir.to_str().unwrap(), args.modules_path.span());
    let mut output = TokenStream::from(quote! {
        /// mod stmts auto generated from path
        static MODULES_DIR: &str = #modules_dir_lit;
    });

    // Recursively walk modules directory and add a hard reference to every single
    let mut static_module_idents = Vec::new();
    let mut static_module_names = Vec::new();
    visit_dirs(&modules_dir, &mut |entry| {
        let p = entry.path();
        if let Some(ext) = p.extension() {
            if ext == "rs" {
                let path_lit = LitStr::new(p.to_str().unwrap(), args.modules_path.span());
                let module_str = p.file_stem().unwrap().to_str().unwrap();
                let module_ident = Ident::new(module_str, args.modules_path.span());
                static_module_names.push(LitStr::new(module_str, args.modules_path.span()));
                static_module_idents.push(module_ident.clone());
                output.extend(TokenStream::from(quote! {
                    #[path = #path_lit]
                    mod #module_ident;
                }));
            }
        } else {
            let path_lit = LitStr::new(entry.path().to_str().unwrap(), args.modules_path.span());
            output.extend(TokenStream::from(quote! {
                #path_lit
            }));
        }
    });

    output.extend(TokenStream::from(quote! {
        fn static_module_handles() -> std::vec::Vec<Box<dyn brane_weaver::ModuleHandle>> {
            let mut modules = std::vec::Vec::new();

            #(
                modules.push(#static_module_idents::allocate(#static_module_names));
            )*

            modules
        }
    }));

    output
}

#[derive(Parse)]
struct MacroArgs {
    mod_ident: Ident,
}

/// passthrough macro for signaling to weaver that we wish to import the module with this name
#[proc_macro]
pub fn import_macro(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as MacroArgs);

    let mod_ident = args.mod_ident;
    TokenStream::from(quote! {
        use #mod_ident;
    })
}
