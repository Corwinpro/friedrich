# Gaussian Process

**This is a work in progress!**

This crate is still in its alpha stage, the interface and internals might still evolve a lot.

My aim is to implement [Gaussian Process Regression](https://en.wikipedia.org/wiki/Gaussian_process) in Rust.

## Usage

The algorithm works on matrices (see [nalgebra](https://www.nalgebra.org/quick_reference/)) of inputs / outputs.

## TODO

- Clean-up the documentation.
- Add better algorithms to fit kernel parameters.
- Add ndarray support behind a feature flag