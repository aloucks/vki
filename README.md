# VKI

[![Build Status](https://dev.azure.com/aloucks/aloucks/_apis/build/status/aloucks.vki?branchName=master)](https://dev.azure.com/aloucks/aloucks/_build/latest?definitionId=1&branchName=master)

VKI is _currently_ a [WebGPU](https://github.com/gpuweb/gpuweb)
implementation, inspired by [WGPU](https://github.com/gfx-rs/wgpu) and
modeled after [Dawn](https://dawn.googlesource.com/dawn).

## Should I use this?

It's not in crates.io at the moment. You probably want to check out
[WGPU](https://github.com/gfx-rs/wgpu-rs),
[Vulkano](https://github.com/vulkano-rs/vulkano), or
[Rendy](https://github.com/amethyst/rendy).

## Does it work?

Yes! See the [examples](examples) directory for more interesting things.

The examples enable the vulkan validation layers which requires the
[Vulkan SDK](https://www.lunarg.com/vulkan-sdk/) to be installed.

A [nuklear-rust backend](https://github.com/aloucks/nuklear-test) for 2D
UI components also works with VKI.

## What does VKI mean?

The `VK` is for Vulkan and the `I` was chosen randomly! I'll probably
rename it eventually.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.