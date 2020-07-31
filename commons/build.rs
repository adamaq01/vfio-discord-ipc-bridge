extern crate vergen;

use vergen::{generate_cargo_keys, ConstantsFlags};

fn main() {
    // Setup the flags, toggling off the 'SEMVER' flag
    let mut flags = ConstantsFlags::all();
    flags.toggle(ConstantsFlags::SEMVER);

    // Generate the 'cargo:' key output
    generate_cargo_keys(flags).expect("Unable to generate the cargo keys!");
}
