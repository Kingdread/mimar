MIMA toolchain
==============

This is an alternative toolchain for the MIMA of the KIT, similar to the one by [cbdevnet][cbdevnet].

`mimar` is written in Rust and compiled using the `cargo` tool.

Compilation
-----------

`cargo build --release` will build the executables as
`./target/release/mimar-{asm,fwc,sim}`. You can copy them to any location you
want.

Documentation
-------------

Not hosted yet, but you can build it locally by running `cargo doc`. To view the
overview, open `./target/doc/mimar/index.html`.

Usage
-----

The usage requires three things:

* A firmware compiled with `mimar-fwc`. You usually do this once, and you
  usually use the default firmware. Only if you want to add new instructions,
  you need to use your own.
* A program assembled with `mimar-asm`. The input *and* output format of
  `mimar-asm` is compatible with `mimasm` from [cbdevnet's simulator][cbdevnet]
  (given you're using the default firmware).
* Simulating with `mimar-sim`, giving both the firmware and the assembled
  program as arguments.

More inforation to the single programs can be found in their documentation.

Example
-------

Compile the default firmware to `firmware`, assemble `input.txt`  and run it:

```
# Output the default and immediately compile it to ./firmware
mimar-fwc --default | mimar-fwc -o firmware
# Assemble the input file to the default out.mima output
mimar-asm firmware input.txt
# Run the simulation, starting at the START label
mimar-sim firmware out.mima -s START
```

License
-------

Distributed under the terms of the MIT license, see [LICENSE.txt](LICENSE.txt).

[cbdevnet]: https://github.com/cbdevnet/mima
