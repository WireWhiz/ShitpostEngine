use brane_weaver::WeaverClient;

proc_brane_weaver::register_all_modules!("../modules");

fn main() {
    let mut client = WeaverClient::new(static_module_handles());

    loop {
        client.update_modules();
        let test_mod = client
            .get_module_by_name("test_mod")
            .expect("Couldn't find test_mod");
        test_mod.tick();
    }
}
