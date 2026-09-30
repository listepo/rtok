//! `~/.rtok/config.toml` text through `rtok config validate` and the file-free config stack
//! (defaults < user TOML < legacy-key fold, `~` expansion, `config show` rows).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    rtok::fuzzing::config_toml(text);
});
